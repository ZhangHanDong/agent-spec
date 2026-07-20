use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use rust_atlas::live::RetryClass;
use rust_atlas::live::{LiveRuntimeState, LiveRuntimeStatus, PendingJournal, RetryPolicy};
use rust_atlas::locking::DaemonLease;
use rust_atlas::scope::AtlasScope;
use rust_atlas::sync::{SyncRequest, sync_once};
use rust_atlas::watch::{AtlasWatcher, DEFAULT_MAX_WATCH_DIRECTORIES};
use serde::{Deserialize, Serialize};

const IDENTITY_SCHEMA: &str = "agent-spec/atlas-daemon-identity-v1";
const PROTOCOL_SCHEMA: &str = "agent-spec/atlas-daemon-protocol-v1";
const PROTOCOL_VERSION: u32 = 1;
const REGISTRY_FILE: &str = "daemon.json";
const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const START_TIMEOUT: Duration = Duration::from_secs(5);
const SYNC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(600);
const DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub(crate) enum DaemonError {
    #[error("atlas-daemon-io: {0}")]
    Io(String),
    #[error("atlas-daemon-identity: {0}")]
    Identity(String),
    #[error("atlas-daemon-protocol: {0}")]
    Protocol(String),
    #[error("atlas-daemon-unavailable: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Atlas(#[from] rust_atlas::AtlasError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DaemonIdentity {
    schema_id: String,
    schema_version: u32,
    worktree_root: String,
    graph_root: String,
    pid: u32,
    started_at_ms: u64,
    startup_nonce: String,
    tool_version: String,
    atlas_schema_version: u32,
    endpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DaemonCommand {
    Status,
    Sync,
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonRequest {
    schema_id: String,
    schema_version: u32,
    identity: DaemonIdentity,
    command: DaemonCommand,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DaemonResponse {
    schema_id: String,
    schema_version: u32,
    pub(crate) identity: DaemonIdentity,
    ok: bool,
    pub(crate) runtime: Option<LiveRuntimeStatus>,
    sync: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ServeOptions {
    auto_sync: bool,
    watch: bool,
    poll_interval: Duration,
}

impl ServeOptions {
    #[cfg(test)]
    fn test_no_runtime() -> Self {
        Self {
            auto_sync: false,
            watch: false,
            poll_interval: Duration::from_millis(5),
        }
    }
}

impl DaemonIdentity {
    fn new(code: &Path, graph: &Path, endpoint: SocketAddr) -> Result<Self, DaemonError> {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|error| DaemonError::Io(error.to_string()))?;
        Self::new_with_nonce(
            code,
            graph,
            std::process::id(),
            hex(&nonce),
            endpoint.to_string(),
        )
    }

    #[cfg(test)]
    fn new_for_test(
        code: &Path,
        graph: &Path,
        pid: u32,
        nonce: String,
        endpoint: &str,
    ) -> Result<Self, DaemonError> {
        Self::new_with_nonce(code, graph, pid, nonce, endpoint.to_string())
    }

    fn new_with_nonce(
        code: &Path,
        graph: &Path,
        pid: u32,
        startup_nonce: String,
        endpoint: String,
    ) -> Result<Self, DaemonError> {
        fs::create_dir_all(graph).map_err(io_error)?;
        let identity = Self {
            schema_id: IDENTITY_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            worktree_root: canonical_string(code)?,
            graph_root: canonical_string(graph)?,
            pid,
            started_at_ms: now_ms()?,
            startup_nonce,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            atlas_schema_version: rust_atlas::SCHEMA_VERSION,
            endpoint,
        };
        identity.validate()?;
        Ok(identity)
    }

    fn validate(&self) -> Result<(), DaemonError> {
        if self.schema_id != IDENTITY_SCHEMA || self.schema_version != PROTOCOL_VERSION {
            return Err(identity_error("unsupported daemon identity schema"));
        }
        if self.pid == 0 || self.started_at_ms == 0 {
            return Err(identity_error("daemon PID and start time must be positive"));
        }
        if self.startup_nonce.len() != 32
            || !self
                .startup_nonce
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(identity_error("startup nonce must be 128-bit hexadecimal"));
        }
        if self.tool_version != env!("CARGO_PKG_VERSION")
            || self.atlas_schema_version != rust_atlas::SCHEMA_VERSION
        {
            return Err(identity_error(
                "daemon tool or Atlas schema version mismatch",
            ));
        }
        for (label, value) in [
            ("worktree root", self.worktree_root.as_str()),
            ("graph root", self.graph_root.as_str()),
        ] {
            if value.is_empty() || value.chars().any(char::is_control) {
                return Err(identity_error(format!("invalid {label}")));
            }
        }
        let endpoint = self
            .endpoint
            .parse::<SocketAddr>()
            .map_err(|error| identity_error(format!("invalid endpoint: {error}")))?;
        if !endpoint.ip().is_loopback() {
            return Err(identity_error("daemon endpoint must be loopback"));
        }
        Ok(())
    }

    fn validate_for(&self, code: &Path, graph: &Path) -> Result<(), DaemonError> {
        self.validate()?;
        let expected_code = canonical_string(code)?;
        fs::create_dir_all(graph).map_err(io_error)?;
        let expected_graph = canonical_string(graph)?;
        if self.worktree_root != expected_code || self.graph_root != expected_graph {
            return Err(identity_error("daemon worktree or graph root mismatch"));
        }
        Ok(())
    }
}

pub(crate) fn serve(code: &Path, graph: &Path) -> Result<(), DaemonError> {
    serve_with_options(
        code,
        graph,
        ServeOptions {
            auto_sync: true,
            watch: true,
            poll_interval: Duration::from_millis(25),
        },
        None,
    )
}

fn serve_with_options(
    code: &Path,
    graph: &Path,
    options: ServeOptions,
    ready: Option<Sender<DaemonIdentity>>,
) -> Result<(), DaemonError> {
    let daemon_lease = DaemonLease::try_acquire(graph)?;
    let code = fs::canonicalize(code).map_err(io_error)?;
    let graph = fs::canonicalize(graph).map_err(io_error)?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(io_error)?;
    listener.set_nonblocking(true).map_err(io_error)?;
    let identity = DaemonIdentity::new(&code, &graph, listener.local_addr().map_err(io_error)?)?;
    write_registry(&graph, &identity)?;

    let mut runtime = LiveRuntimeStatus::new(LiveRuntimeState::Warming);
    runtime.store(&graph)?;
    let mut watcher = if options.watch {
        let scope = AtlasScope::discover(&code, &graph)?;
        let journal = PendingJournal::open(&graph)?;
        match AtlasWatcher::start(scope, journal, DEFAULT_MAX_WATCH_DIRECTORIES) {
            Ok(watcher) if watcher.plan().is_healthy() => {
                runtime.watch_healthy = Some(true);
                runtime.store(&graph)?;
                Some(watcher)
            }
            Ok(watcher) => {
                runtime.watch_healthy = Some(false);
                runtime.watch_diagnostic =
                    Some(format!("watch coverage: {:?}", watcher.plan().coverage));
                runtime.state = LiveRuntimeState::Degraded;
                runtime.diagnostics = vec![runtime.watch_diagnostic.clone().unwrap_or_default()];
                runtime.store(&graph)?;
                Some(watcher)
            }
            Err(error) => {
                runtime.watch_healthy = Some(false);
                runtime.watch_diagnostic = Some(error.to_string());
                runtime.state = LiveRuntimeState::Degraded;
                runtime.diagnostics = vec![error.to_string()];
                runtime.store(&graph)?;
                None
            }
        }
    } else {
        None
    };
    if let Some(ready) = ready {
        let _ = ready.send(identity.clone());
    }

    let mut stopping = false;
    let mut next_sync = options.auto_sync.then(Instant::now);
    while !stopping {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Ok(stop) = handle_connection(stream, &identity, &code, &graph) {
                        stopping |= stop;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(io_error(error)),
            }
        }
        if let Some(watcher) = &mut watcher {
            match watcher.drain(now_ms()?) {
                Ok(drain) => {
                    if drain.recorded > 0 && options.auto_sync {
                        next_sync = Some(Instant::now() + DEBOUNCE);
                    }
                    if !matches!(drain.coverage, rust_atlas::watch::WatchCoverage::Complete) {
                        let mut status = LiveRuntimeStatus::load(&graph)?;
                        status.watch_healthy = Some(false);
                        status.watch_diagnostic =
                            Some(format!("watch coverage: {:?}", drain.coverage));
                        status.state = LiveRuntimeState::Degraded;
                        status.diagnostics =
                            vec![status.watch_diagnostic.clone().unwrap_or_default()];
                        status.store(&graph)?;
                    } else {
                        let mut status = LiveRuntimeStatus::load(&graph)?;
                        status.watch_healthy = Some(true);
                        status.watch_diagnostic = None;
                        if status.state == LiveRuntimeState::Degraded && !status.retry.is_degraded()
                        {
                            status.state = if status.pending_paths.is_empty() {
                                LiveRuntimeState::Healthy
                            } else {
                                LiveRuntimeState::Pending
                            };
                            status.diagnostics.clear();
                        }
                        status.store(&graph)?;
                    }
                }
                Err(error) => {
                    let mut status = LiveRuntimeStatus::load(&graph)?;
                    status.watch_healthy = Some(false);
                    status.watch_diagnostic = Some(error.to_string());
                    status.state = LiveRuntimeState::Degraded;
                    status.diagnostics = vec![error.to_string()];
                    status.store(&graph)?;
                }
            }
        }
        if next_sync.is_some_and(|due| Instant::now() >= due) {
            match sync_once(SyncRequest {
                code_root: &code,
                graph_root: &graph,
                build_options: &rust_atlas::BuildOptions::default(),
            }) {
                Ok(_) => next_sync = None,
                Err(error) => {
                    let status = LiveRuntimeStatus::load(&graph)?;
                    if status.state == LiveRuntimeState::Degraded {
                        next_sync = None;
                    } else {
                        let attempts = if matches!(error, rust_atlas::AtlasError::WriterBusy { .. })
                        {
                            status.retry.writer_lock_attempts
                        } else {
                            status.retry.ordinary_attempts
                        };
                        next_sync = RetryPolicy::default()
                            .delay_for_attempt(attempts)?
                            .map(|delay| Instant::now() + Duration::from_millis(delay));
                    }
                }
            }
        }
        if !stopping {
            std::thread::sleep(options.poll_interval);
        }
    }

    let mut stopped = LiveRuntimeStatus::load(&graph)?;
    stopped.state = LiveRuntimeState::Unavailable;
    stopped.store(&graph)?;
    remove_registry_if_owned(&graph, &identity);
    drop(watcher);
    drop(daemon_lease);
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    identity: &DaemonIdentity,
    code: &Path,
    graph: &Path,
) -> Result<bool, DaemonError> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(io_error)?;
    let Some(request) = read_line_json::<DaemonRequest>(&mut stream)? else {
        return Ok(false);
    };
    if request.schema_id != PROTOCOL_SCHEMA
        || request.schema_version != PROTOCOL_VERSION
        || request.identity != *identity
    {
        write_response(
            &mut stream,
            &DaemonResponse::error(identity.clone(), "daemon identity handshake mismatch"),
        )?;
        return Ok(false);
    }
    match request.command {
        DaemonCommand::Status => {
            let runtime = LiveRuntimeStatus::load(graph)?;
            write_response(
                &mut stream,
                &DaemonResponse::status(identity.clone(), runtime),
            )?;
            Ok(false)
        }
        DaemonCommand::Sync => match sync_once(SyncRequest {
            code_root: code,
            graph_root: graph,
            build_options: &rust_atlas::BuildOptions::default(),
        }) {
            Ok(receipt) => {
                let runtime = receipt.runtime.clone();
                let sync = serde_json::to_value(receipt)
                    .map_err(|error| DaemonError::Protocol(error.to_string()))?;
                write_response(
                    &mut stream,
                    &DaemonResponse::sync(identity.clone(), runtime, sync),
                )?;
                Ok(false)
            }
            Err(error) => {
                write_response(
                    &mut stream,
                    &DaemonResponse::error(identity.clone(), &error.to_string()),
                )?;
                Ok(false)
            }
        },
        DaemonCommand::Stop => {
            let runtime = LiveRuntimeStatus::load(graph)?;
            write_response(
                &mut stream,
                &DaemonResponse::status(identity.clone(), runtime),
            )?;
            Ok(true)
        }
    }
}

impl DaemonResponse {
    fn status(identity: DaemonIdentity, runtime: LiveRuntimeStatus) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: true,
            runtime: Some(runtime),
            sync: None,
            error: None,
        }
    }

    fn sync(identity: DaemonIdentity, runtime: LiveRuntimeStatus, sync: serde_json::Value) -> Self {
        Self {
            sync: Some(sync),
            ..Self::status(identity, runtime)
        }
    }

    fn error(identity: DaemonIdentity, error: &str) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: false,
            runtime: None,
            sync: None,
            error: Some(error.to_string()),
        }
    }
}

pub(crate) fn start(code: &Path, graph: &Path) -> Result<LiveRuntimeStatus, DaemonError> {
    fs::create_dir_all(graph).map_err(io_error)?;
    let code = fs::canonicalize(code).map_err(io_error)?;
    let graph = fs::canonicalize(graph).map_err(io_error)?;
    if let Ok(status) = live_daemon_status(&code, &graph) {
        return Ok(status);
    }
    let lease = match DaemonLease::try_acquire(&graph) {
        Ok(lease) => Some(lease),
        Err(rust_atlas::AtlasError::DaemonBusy { .. }) => None,
        Err(error) => return Err(error.into()),
    };
    if lease.is_none() {
        let deadline = Instant::now() + START_TIMEOUT;
        while Instant::now() < deadline {
            if let Ok(status) = live_daemon_status(&code, &graph) {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        return Err(identity_error(
            "active daemon did not prove the requested root, version and startup identity",
        ));
    }
    drop(lease);
    let executable = std::env::current_exe().map_err(io_error)?;
    let mut command = detached_command(&executable);
    command
        .args(["atlas", "daemon", "serve", "--code"])
        .arg(&code)
        .arg("--graph")
        .arg(&graph)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(io_error)?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(status) = live_daemon_status(&code, &graph) {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(DaemonError::Unavailable(
        "daemon did not complete a verified handshake within 5 seconds".to_string(),
    ))
}

fn detached_command(executable: &Path) -> Command {
    #[cfg(unix)]
    {
        let mut command = Command::new("nohup");
        command.arg(executable);
        command
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let mut command = Command::new(executable);
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        command
    }
    #[cfg(not(any(unix, windows)))]
    {
        Command::new(executable)
    }
}

pub(crate) fn daemon_status(code: &Path, graph: &Path) -> Result<LiveRuntimeStatus, DaemonError> {
    match live_daemon_status(code, graph) {
        Ok(status) => Ok(status),
        Err(DaemonError::Unavailable(_)) => {
            let mut status = LiveRuntimeStatus::load(graph)
                .unwrap_or_else(|_| LiveRuntimeStatus::new(LiveRuntimeState::Unavailable));
            status.state = LiveRuntimeState::Unavailable;
            Ok(status)
        }
        Err(error) => Err(error),
    }
}

fn live_daemon_status(code: &Path, graph: &Path) -> Result<LiveRuntimeStatus, DaemonError> {
    let response = send_command(code, graph, DaemonCommand::Status)?;
    response
        .runtime
        .ok_or_else(|| DaemonError::Protocol("status response omitted runtime".to_string()))
}

pub(crate) fn sync(code: &Path, graph: &Path) -> Result<DaemonResponse, DaemonError> {
    send_command(code, graph, DaemonCommand::Sync)
}

pub(crate) fn stop(code: &Path, graph: &Path) -> Result<DaemonResponse, DaemonError> {
    send_command(code, graph, DaemonCommand::Stop)
}

fn send_command(
    code: &Path,
    graph: &Path,
    command: DaemonCommand,
) -> Result<DaemonResponse, DaemonError> {
    let identity = read_registry(graph)?;
    send_to_identity(&identity, code, graph, command)
}

fn send_to_identity(
    identity: &DaemonIdentity,
    code: &Path,
    graph: &Path,
    command: DaemonCommand,
) -> Result<DaemonResponse, DaemonError> {
    identity.validate_for(code, graph)?;
    let endpoint = identity
        .endpoint
        .parse::<SocketAddr>()
        .map_err(|error| identity_error(error.to_string()))?;
    let mut stream = TcpStream::connect_timeout(&endpoint, IO_TIMEOUT)
        .map_err(|error| DaemonError::Unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(command_read_timeout(command)))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(io_error)?;
    let request = DaemonRequest {
        schema_id: PROTOCOL_SCHEMA.to_string(),
        schema_version: PROTOCOL_VERSION,
        identity: identity.clone(),
        command,
    };
    write_json_line(&mut stream, &request)?;
    let response = read_line_json::<DaemonResponse>(&mut stream)?
        .ok_or_else(|| DaemonError::Protocol("daemon closed without a response".to_string()))?;
    if response.schema_id != PROTOCOL_SCHEMA
        || response.schema_version != PROTOCOL_VERSION
        || response.identity != *identity
    {
        return Err(identity_error("daemon response identity mismatch"));
    }
    if !response.ok {
        return Err(DaemonError::Protocol(
            response
                .error
                .unwrap_or_else(|| "daemon command failed".to_string()),
        ));
    }
    Ok(response)
}

fn command_read_timeout(command: DaemonCommand) -> Duration {
    match command {
        DaemonCommand::Status | DaemonCommand::Stop => IO_TIMEOUT,
        DaemonCommand::Sync => SYNC_RESPONSE_TIMEOUT,
    }
}

fn registry_path(graph: &Path) -> PathBuf {
    graph.join(".runtime").join(REGISTRY_FILE)
}

fn write_registry(graph: &Path, identity: &DaemonIdentity) -> Result<(), DaemonError> {
    identity.validate()?;
    let path = registry_path(graph);
    let parent = path
        .parent()
        .ok_or_else(|| DaemonError::Io("registry has no parent".to_string()))?;
    fs::create_dir_all(parent).map_err(io_error)?;
    reject_symlink(&path)?;
    let bytes = serde_json::to_vec_pretty(identity)
        .map_err(|error| DaemonError::Protocol(error.to_string()))?;
    if bytes.len() > MAX_PROTOCOL_BYTES {
        return Err(DaemonError::Protocol(
            "daemon registry exceeds 1 MiB".to_string(),
        ));
    }
    let temporary = parent.join(format!(
        ".{REGISTRY_FILE}.tmp-{}-{}",
        std::process::id(),
        identity.startup_nonce
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(io_error)?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(io_error(error));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(io_error(error));
    }
    Ok(())
}

fn read_registry(graph: &Path) -> Result<DaemonIdentity, DaemonError> {
    let path = registry_path(graph);
    reject_symlink(&path)?;
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DaemonError::Unavailable(
                "daemon registry is absent".to_string(),
            ));
        }
        Err(error) => return Err(io_error(error)),
    };
    if metadata.len() > MAX_PROTOCOL_BYTES as u64 {
        return Err(DaemonError::Protocol(
            "daemon registry exceeds 1 MiB".to_string(),
        ));
    }
    let bytes = fs::read(&path).map_err(io_error)?;
    let identity: DaemonIdentity = serde_json::from_slice(&bytes)
        .map_err(|error| DaemonError::Protocol(format!("invalid daemon registry: {error}")))?;
    identity.validate()?;
    Ok(identity)
}

fn remove_registry_if_owned(graph: &Path, identity: &DaemonIdentity) {
    if read_registry(graph).is_ok_and(|current| current == *identity) {
        let _ = fs::remove_file(registry_path(graph));
    }
}

fn write_response(stream: &mut TcpStream, response: &DaemonResponse) -> Result<(), DaemonError> {
    write_json_line(stream, response)
}

fn write_json_line<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    if bytes.len() >= MAX_PROTOCOL_BYTES {
        return Err(DaemonError::Protocol(
            "daemon message exceeds 1 MiB".to_string(),
        ));
    }
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(io_error)
}

fn read_line_json<T: for<'de> Deserialize<'de>>(
    stream: &mut TcpStream,
) -> Result<Option<T>, DaemonError> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    let read = reader
        .by_ref()
        .take((MAX_PROTOCOL_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(io_error)?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_PROTOCOL_BYTES || bytes.last() != Some(&b'\n') {
        return Err(DaemonError::Protocol(
            "daemon message is unterminated or exceeds 1 MiB".to_string(),
        ));
    }
    let value = serde_json::from_slice(&bytes[..bytes.len() - 1])
        .map_err(|error| DaemonError::Protocol(format!("invalid daemon message: {error}")))?;
    Ok(Some(value))
}

fn reject_symlink(path: &Path) -> Result<(), DaemonError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(identity_error(format!(
            "daemon state path must not be a symlink: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

fn canonical_string(path: &Path) -> Result<String, DaemonError> {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(io_error)
}

fn now_ms() -> Result<u64, DaemonError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| DaemonError::Io(error.to_string()))?
        .as_millis();
    u64::try_from(millis).map_err(|_| DaemonError::Io("system time exceeds u64".to_string()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn identity_error(detail: impl Into<String>) -> DaemonError {
    DaemonError::Identity(detail.into())
}

fn io_error(error: std::io::Error) -> DaemonError {
    DaemonError::Io(error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::net::TcpStream;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    fn fixture(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "agent-spec-daemon-{name}-{}-{}",
            std::process::id(),
            now_ms().unwrap()
        ));
        let code = root.join("code");
        let graph = root.join("graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"daemon-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        (code, graph)
    }

    fn spawn_test_server(code: &Path, graph: &Path) -> (DaemonIdentity, thread::JoinHandle<()>) {
        let (ready_tx, ready_rx) = mpsc::channel();
        let code = code.to_path_buf();
        let graph = graph.to_path_buf();
        let handle = thread::spawn(move || {
            serve_with_options(
                &code,
                &graph,
                ServeOptions {
                    auto_sync: false,
                    watch: false,
                    poll_interval: Duration::from_millis(5),
                },
                Some(ready_tx),
            )
            .unwrap();
        });
        (
            ready_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            handle,
        )
    }

    fn stop_server(code: &Path, graph: &Path, handle: thread::JoinHandle<()>) {
        send_command(code, graph, DaemonCommand::Stop).unwrap();
        handle.join().unwrap();
        assert!(!registry_path(graph).exists());
    }

    #[test]
    fn test_atlas_daemon_sync_uses_bounded_long_operation_timeout() {
        assert_eq!(command_read_timeout(DaemonCommand::Status), IO_TIMEOUT);
        assert_eq!(command_read_timeout(DaemonCommand::Stop), IO_TIMEOUT);
        assert_eq!(
            command_read_timeout(DaemonCommand::Sync),
            Duration::from_secs(600)
        );
        assert!(command_read_timeout(DaemonCommand::Sync) > START_TIMEOUT);
    }

    #[test]
    fn test_atlas_daemon_identity_rejects_wrong_root_version_and_nonce() {
        let (code, graph) = fixture("identity");
        let (identity, handle) = spawn_test_server(&code, &graph);

        let mut wrong_root = identity.clone();
        wrong_root.worktree_root = code.join("other").display().to_string();
        assert!(send_to_identity(&wrong_root, &code, &graph, DaemonCommand::Status).is_err());
        let mut wrong_graph = identity.clone();
        wrong_graph.graph_root = graph.join("other").display().to_string();
        assert!(send_to_identity(&wrong_graph, &code, &graph, DaemonCommand::Status).is_err());
        let mut wrong_version = identity.clone();
        wrong_version.tool_version = "0.0.0-wrong".to_string();
        assert!(send_to_identity(&wrong_version, &code, &graph, DaemonCommand::Status).is_err());
        let mut wrong_schema = identity.clone();
        wrong_schema.atlas_schema_version += 1;
        assert!(send_to_identity(&wrong_schema, &code, &graph, DaemonCommand::Status).is_err());
        let mut wrong_nonce = identity.clone();
        wrong_nonce.startup_nonce = "00".repeat(16);
        assert!(send_to_identity(&wrong_nonce, &code, &graph, DaemonCommand::Status).is_err());

        assert_eq!(
            send_command(&code, &graph, DaemonCommand::Status)
                .unwrap()
                .identity,
            identity
        );
        stop_server(&code, &graph, handle);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_single_writer_recovers_stale_registry_and_pid_reuse() {
        let (code, graph) = fixture("singleton");
        fs::create_dir_all(graph.join(".runtime")).unwrap();
        let stale = DaemonIdentity::new_for_test(
            &code,
            &graph,
            std::process::id(),
            "11".repeat(16),
            "127.0.0.1:9",
        )
        .unwrap();
        write_registry(&graph, &stale).unwrap();

        let (live, handle) = spawn_test_server(&code, &graph);
        assert_ne!(live.startup_nonce, stale.startup_nonce);
        assert_eq!(live.pid, stale.pid);
        let second =
            serve_with_options(&code, &graph, ServeOptions::test_no_runtime(), None).unwrap_err();
        assert!(second.to_string().contains("already active"));
        assert_eq!(read_registry(&graph).unwrap(), live);

        stop_server(&code, &graph, handle);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_client_disconnect_does_not_stop_server() {
        let (code, graph) = fixture("disconnect");
        let (identity, handle) = spawn_test_server(&code, &graph);
        drop(TcpStream::connect(&identity.endpoint).unwrap());
        thread::sleep(Duration::from_millis(30));

        let response = send_command(&code, &graph, DaemonCommand::Status).unwrap();
        assert_eq!(response.identity, identity);
        assert!(response.runtime.is_some());
        stop_server(&code, &graph, handle);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_reports_warming_pending_degraded_unavailable() {
        let (code, graph) = fixture("states");
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Unavailable
        );
        let (_identity, handle) = spawn_test_server(&code, &graph);
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Warming
        );

        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 1).unwrap();
        let mut pending = LiveRuntimeStatus::new(LiveRuntimeState::Pending);
        pending.pending_paths = vec!["src/lib.rs".to_string()];
        pending.store(&graph).unwrap();
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Pending
        );

        for _ in 0..6 {
            pending
                .retry
                .record_failure(
                    RetryClass::Ordinary,
                    "injected failure",
                    &RetryPolicy::default(),
                )
                .unwrap();
        }
        pending.state = LiveRuntimeState::Degraded;
        pending.store(&graph).unwrap();
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Degraded
        );

        pending.retry.reset_after_success();
        journal
            .acknowledge(&journal.snapshot().unwrap(), &Default::default())
            .unwrap();
        pending.pending_paths.clear();
        pending.state = LiveRuntimeState::Healthy;
        pending.store(&graph).unwrap();
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Healthy
        );

        stop_server(&code, &graph, handle);
        assert_eq!(
            daemon_status(&code, &graph).unwrap().state,
            LiveRuntimeState::Unavailable
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }
}
