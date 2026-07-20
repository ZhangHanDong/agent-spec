use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use rust_atlas::live::RetryClass;
use rust_atlas::live::{LiveRuntimeState, LiveRuntimeStatus, PendingJournal, RetryPolicy};
use rust_atlas::locking::DaemonLease;
use rust_atlas::scope::AtlasScope;
use rust_atlas::sync::{SyncRequest, sync_once};
use rust_atlas::watch::{AtlasWatcher, DEFAULT_MAX_WATCH_DIRECTORIES};
use serde::{Deserialize, Serialize};

const IDENTITY_SCHEMA: &str = "agent-spec/atlas-daemon-identity-v2";
const PROTOCOL_SCHEMA: &str = "agent-spec/atlas-daemon-protocol-v2";
const PROTOCOL_VERSION: u32 = 2;
const REGISTRY_FILE: &str = "daemon.json";
const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const START_TIMEOUT: Duration = Duration::from_secs(5);
const SYNC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(600);
const DEBOUNCE: Duration = Duration::from_millis(100);
const MAX_PENDING_IO: usize = 256;
const CONTROL_IO_RESERVE: usize = 16;
const IO_CONNECTION_BUDGET: usize = 64;
const IO_BYTE_BUDGET: usize = 64 * 1024;
const MIN_BULK_CONNECTION_BUDGET: usize = 8;
const MIN_BULK_BYTE_BUDGET: usize = 8 * 1024;

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
#[serde(rename_all = "kebab-case")]
pub(crate) enum DaemonCommand {
    Status,
    Sync,
    Stop,
    Context,
    Cancel,
    ServiceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonContextRequest {
    request_id: String,
    query: String,
    profile: rust_atlas::ContextProfile,
    #[serde(default)]
    max_serialized_bytes: Option<usize>,
    #[serde(default)]
    min_score: Option<u16>,
    #[serde(default)]
    after: Option<String>,
    #[serde(default)]
    expected_graph_fingerprint: Option<String>,
    #[serde(default)]
    failure_evidence: Vec<String>,
}

impl DaemonContextRequest {
    fn options(&self) -> rust_atlas::ContextOptions {
        rust_atlas::ContextOptions {
            profile: self.profile,
            frozen: true,
            max_serialized_bytes: self.max_serialized_bytes,
            min_score: self.min_score,
            after: self.after.clone(),
            expected_graph_fingerprint: self.expected_graph_fingerprint.clone(),
            failure_evidence: self.failure_evidence.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonCancelRequest {
    request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonCancelResponse {
    request_id: String,
    cancelled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonRequest {
    schema_id: String,
    schema_version: u32,
    identity: DaemonIdentity,
    command: DaemonCommand,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<DaemonContextRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cancel: Option<DaemonCancelRequest>,
}

impl DaemonRequest {
    fn command(identity: DaemonIdentity, command: DaemonCommand) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            command,
            context: None,
            cancel: None,
        }
    }

    fn validate_payload(&self) -> Result<(), DaemonError> {
        let valid = match self.command {
            DaemonCommand::Status
            | DaemonCommand::Sync
            | DaemonCommand::Stop
            | DaemonCommand::ServiceStatus => self.context.is_none() && self.cancel.is_none(),
            DaemonCommand::Context => self.context.is_some() && self.cancel.is_none(),
            DaemonCommand::Cancel => self.context.is_none() && self.cancel.is_some(),
        };
        if !valid {
            return Err(DaemonError::Protocol(
                "daemon command payload does not match command".to_string(),
            ));
        }
        let request_id = self
            .context
            .as_ref()
            .map(|payload| payload.request_id.as_str())
            .or_else(|| {
                self.cancel
                    .as_ref()
                    .map(|payload| payload.request_id.as_str())
            });
        if request_id.is_some_and(|value| !crate::atlas_query_service::valid_request_id(value)) {
            return Err(DaemonError::Protocol(
                "atlas-query-request-id: expected 1..=64 ASCII alphanumeric, '-' or '_'"
                    .to_string(),
            ));
        }
        Ok(())
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<crate::atlas_query_service::QueryServiceWireReply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<crate::atlas_query_service::QueryServiceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancel: Option<DaemonCancelResponse>,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ServeOptions {
    auto_sync: bool,
    watch: bool,
    poll_interval: Duration,
    query_config: crate::atlas_query_service::QueryServiceConfig,
}

impl ServeOptions {
    #[cfg(test)]
    fn test_no_runtime() -> Self {
        Self {
            auto_sync: false,
            watch: false,
            poll_interval: Duration::from_millis(5),
            query_config: crate::atlas_query_service::QueryServiceConfig::default(),
        }
    }
}

struct MaintenanceSuccess {
    runtime: LiveRuntimeStatus,
    sync: serde_json::Value,
}

trait MaintenanceRunner: Send + Sync + 'static {
    fn run(
        &self,
        code: &Path,
        graph: &Path,
        cancellation: Arc<AtomicBool>,
    ) -> Result<MaintenanceSuccess, String>;
}

struct SyncOnceRunner;

impl MaintenanceRunner for SyncOnceRunner {
    fn run(
        &self,
        code: &Path,
        graph: &Path,
        cancellation: Arc<AtomicBool>,
    ) -> Result<MaintenanceSuccess, String> {
        let build_options = rust_atlas::BuildOptions {
            cancellation: Some(cancellation),
            ..rust_atlas::BuildOptions::default()
        };
        sync_once(SyncRequest {
            code_root: code,
            graph_root: graph,
            build_options: &build_options,
        })
        .and_then(|receipt| {
            let runtime = receipt.runtime.clone();
            serde_json::to_value(receipt)
                .map(|sync| MaintenanceSuccess { runtime, sync })
                .map_err(|error| rust_atlas::AtlasError::Invariant(error.to_string()))
        })
        .map_err(|error| error.to_string())
    }
}

struct MaintenanceJob {
    token: Option<u64>,
}

struct MaintenanceCompletion {
    token: Option<u64>,
    result: Result<MaintenanceSuccess, String>,
}

struct MaintenanceLane {
    sender: Option<SyncSender<MaintenanceJob>>,
    completions: Receiver<MaintenanceCompletion>,
    cancellation: Arc<AtomicBool>,
    outstanding: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

struct PendingResponses {
    queries: BTreeMap<String, TcpStream>,
    sync: BTreeMap<u64, TcpStream>,
    inbound: VecDeque<InboundConnection>,
    control_outbound: VecDeque<OutboundConnection>,
    bulk_outbound: VecDeque<OutboundConnection>,
    next_maintenance_token: u64,
}

struct InboundConnection {
    stream: TcpStream,
    bytes: Vec<u8>,
    accepted_at: Instant,
}

struct OutboundConnection {
    stream: TcpStream,
    bytes: Vec<u8>,
    offset: usize,
    last_progress_at: Instant,
}

impl PendingResponses {
    fn new() -> Self {
        Self {
            queries: BTreeMap::new(),
            sync: BTreeMap::new(),
            inbound: VecDeque::new(),
            control_outbound: VecDeque::new(),
            bulk_outbound: VecDeque::new(),
            next_maintenance_token: 1,
        }
    }

    fn io_count(&self) -> usize {
        self.inbound
            .len()
            .saturating_add(self.control_outbound.len())
            .saturating_add(self.bulk_outbound.len())
            .saturating_add(self.queries.len())
            .saturating_add(self.sync.len())
    }

    fn has_accept_capacity(&self) -> bool {
        self.io_count() < MAX_PENDING_IO.saturating_add(CONTROL_IO_RESERVE)
    }

    fn has_bulk_capacity(&self) -> bool {
        self.queries
            .len()
            .saturating_add(self.sync.len())
            .saturating_add(self.bulk_outbound.len())
            < MAX_PENDING_IO
    }

    fn queue_control(
        &mut self,
        stream: TcpStream,
        response: &DaemonResponse,
    ) -> Result<(), DaemonError> {
        let bytes = encode_json_line(response)?;
        self.queue_outbound(stream, bytes, true)
    }

    fn queue_bulk(
        &mut self,
        stream: TcpStream,
        response: &DaemonResponse,
    ) -> Result<(), DaemonError> {
        let bytes = encode_json_line(response)?;
        self.queue_outbound(stream, bytes, false)
    }

    fn queue_query(
        &mut self,
        stream: TcpStream,
        identity: &DaemonIdentity,
        reply: &crate::atlas_query_service::QueryServiceReply,
    ) -> Result<(), DaemonError> {
        let bytes = encode_query_reply(identity, reply)?;
        self.queue_outbound(stream, bytes, false)
    }

    fn queue_outbound(
        &mut self,
        stream: TcpStream,
        bytes: Vec<u8>,
        control: bool,
    ) -> Result<(), DaemonError> {
        let capacity = if control {
            self.has_accept_capacity()
        } else {
            self.has_bulk_capacity()
        };
        if !capacity {
            return Err(DaemonError::Unavailable(
                "atlas-daemon-busy: pending socket capacity is full".to_string(),
            ));
        }
        let connection = OutboundConnection {
            stream,
            bytes,
            offset: 0,
            last_progress_at: Instant::now(),
        };
        if control {
            self.control_outbound.push_back(connection);
        } else {
            self.bulk_outbound.push_back(connection);
        }
        Ok(())
    }

    fn drain_outbound(&mut self) {
        let control_connections = IO_CONNECTION_BUDGET - MIN_BULK_CONNECTION_BUDGET;
        let control_bytes = IO_BYTE_BUDGET - MIN_BULK_BYTE_BUDGET;
        let mut remaining_control_connections = control_connections;
        let mut remaining_control_bytes = control_bytes;
        drain_outbound_queue(
            &mut self.control_outbound,
            &mut remaining_control_connections,
            &mut remaining_control_bytes,
        );
        let used_connections = control_connections - remaining_control_connections;
        let used_bytes = control_bytes - remaining_control_bytes;
        let mut bulk_connections = IO_CONNECTION_BUDGET - used_connections;
        let mut bulk_bytes = IO_BYTE_BUDGET - used_bytes;
        drain_outbound_queue(
            &mut self.bulk_outbound,
            &mut bulk_connections,
            &mut bulk_bytes,
        );
    }
}

impl MaintenanceLane {
    fn new(code: PathBuf, graph: PathBuf, runner: Arc<dyn MaintenanceRunner>) -> Self {
        let (sender, receiver) = sync_channel::<MaintenanceJob>(1);
        let (completion_sender, completions) = sync_channel::<MaintenanceCompletion>(2);
        let cancellation = Arc::new(AtomicBool::new(false));
        let outstanding = Arc::new(AtomicUsize::new(0));
        let worker_cancellation = Arc::clone(&cancellation);
        let worker_outstanding = Arc::clone(&outstanding);
        let worker = std::thread::Builder::new()
            .name("atlas-maintenance".to_string())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        runner.run(&code, &graph, Arc::clone(&worker_cancellation))
                    }))
                    .unwrap_or_else(|_| {
                        Err("atlas-maintenance-failed: worker panicked".to_string())
                    });
                    worker_outstanding.fetch_sub(1, Ordering::AcqRel);
                    if completion_sender
                        .send(MaintenanceCompletion {
                            token: job.token,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .ok();
        Self {
            sender: worker.as_ref().map(|_| sender),
            completions,
            cancellation,
            outstanding,
            worker,
        }
    }

    fn try_submit(&self, token: Option<u64>) -> Result<(), &'static str> {
        let Some(sender) = self.sender.as_ref() else {
            return Err("atlas-maintenance-unavailable: worker is not running");
        };
        self.outstanding.fetch_add(1, Ordering::AcqRel);
        match sender.try_send(MaintenanceJob { token }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.outstanding.fetch_sub(1, Ordering::AcqRel);
                Err("atlas-maintenance-busy: queue is full")
            }
            Err(TrySendError::Disconnected(_)) => {
                self.outstanding.fetch_sub(1, Ordering::AcqRel);
                Err("atlas-maintenance-unavailable: queue is disconnected")
            }
        }
    }

    fn try_completion(&self) -> Option<MaintenanceCompletion> {
        self.completions.try_recv().ok()
    }

    fn has_outstanding(&self) -> bool {
        self.outstanding.load(Ordering::Acquire) > 0
    }

    fn begin_shutdown(&mut self) {
        self.cancellation.store(true, Ordering::Release);
        self.sender.take();
    }

    fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    fn shutdown(&mut self) {
        self.begin_shutdown();
        self.join();
    }
}

impl Drop for MaintenanceLane {
    fn drop(&mut self) {
        self.shutdown();
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
            query_config: crate::atlas_query_service::QueryServiceConfig::default(),
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
    serve_with_runners(code, graph, options, ready, None, None)
}

fn serve_with_runners(
    code: &Path,
    graph: &Path,
    options: ServeOptions,
    ready: Option<Sender<DaemonIdentity>>,
    query_runner: Option<Arc<dyn crate::atlas_query_service::QueryRunner>>,
    maintenance_runner: Option<Arc<dyn MaintenanceRunner>>,
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
    let query_runner = query_runner.unwrap_or_else(|| {
        Arc::new(crate::atlas_query_service::PinnedContextRunner::new(
            &code, &graph,
        ))
    });
    let mut query_service =
        crate::atlas_query_service::QueryService::new(options.query_config, query_runner)
            .map_err(|error| DaemonError::Protocol(error.to_string()))?;
    let maintenance_runner = maintenance_runner.unwrap_or_else(|| Arc::new(SyncOnceRunner));
    let mut maintenance = MaintenanceLane::new(code.clone(), graph.clone(), maintenance_runner);
    if let Some(ready) = ready {
        let _ = ready.send(identity.clone());
    }

    let mut stopping = false;
    let mut next_sync = options.auto_sync.then(Instant::now);
    let mut pending = PendingResponses::new();
    while !stopping {
        while let Some(reply) = query_service.try_completion() {
            if let Some(stream) = pending.queries.remove(&reply.receipt.request_id) {
                let _ = pending.queue_query(stream, &identity, &reply);
            }
        }
        while let Some(completion) = maintenance.try_completion() {
            let is_auto = completion.token.is_none();
            match completion.result {
                Ok(success) => {
                    if let Some(token) = completion.token
                        && let Some(stream) = pending.sync.remove(&token)
                    {
                        let _ = pending.queue_bulk(
                            stream,
                            &DaemonResponse::sync(identity.clone(), success.runtime, success.sync),
                        );
                    }
                }
                Err(error) => {
                    if let Some(token) = completion.token
                        && let Some(stream) = pending.sync.remove(&token)
                    {
                        let _ = pending
                            .queue_bulk(stream, &DaemonResponse::error(identity.clone(), &error));
                    }
                    if is_auto {
                        next_sync = maintenance_retry_at(&graph, &error)?;
                    }
                }
            }
        }
        for _ in 0..IO_CONNECTION_BUDGET {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true).map_err(io_error)?;
                    if pending.has_accept_capacity() {
                        pending.inbound.push_back(InboundConnection {
                            stream,
                            bytes: Vec::new(),
                            accepted_at: Instant::now(),
                        });
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(io_error(error)),
            }
        }
        let inbound_count = pending.inbound.len().min(IO_CONNECTION_BUDGET);
        let mut read_budget = IO_BYTE_BUDGET;
        for _ in 0..inbound_count {
            let Some(mut connection) = pending.inbound.pop_front() else {
                break;
            };
            match poll_inbound(&mut connection, &mut read_budget) {
                InboundPoll::Pending => pending.inbound.push_back(connection),
                InboundPoll::Closed => {}
                InboundPoll::Error(error) => {
                    let _ = pending.queue_control(
                        connection.stream,
                        &DaemonResponse::error(identity.clone(), &error),
                    );
                }
                InboundPoll::Request(request) => {
                    if let Ok(stop) = handle_request(
                        connection.stream,
                        *request,
                        &identity,
                        &graph,
                        &mut query_service,
                        &mut maintenance,
                        &mut pending,
                    ) {
                        stopping |= stop;
                    }
                }
            }
            if stopping || read_budget == 0 {
                break;
            }
        }
        if !stopping && let Some(watcher) = &mut watcher {
            match watcher.drain(now_ms()?) {
                Ok(drain) => {
                    if drain.recorded > 0 && options.auto_sync {
                        next_sync = Some(Instant::now() + DEBOUNCE);
                    }
                    if !maintenance.has_outstanding() {
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
                            if status.state == LiveRuntimeState::Degraded
                                && !status.retry.is_degraded()
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
                }
                Err(error) => {
                    if !maintenance.has_outstanding() {
                        let mut status = LiveRuntimeStatus::load(&graph)?;
                        status.watch_healthy = Some(false);
                        status.watch_diagnostic = Some(error.to_string());
                        status.state = LiveRuntimeState::Degraded;
                        status.diagnostics = vec![error.to_string()];
                        status.store(&graph)?;
                    }
                }
            }
        }
        if !stopping && next_sync.is_some_and(|due| Instant::now() >= due) {
            match maintenance.try_submit(None) {
                Ok(()) => next_sync = None,
                Err(error) if error.contains("busy") => {
                    next_sync = Some(Instant::now() + options.poll_interval);
                }
                Err(error) => return Err(DaemonError::Unavailable(error.to_string())),
            }
        }
        pending.drain_outbound();
        if !stopping {
            std::thread::sleep(options.poll_interval);
        }
    }

    if let Some(active_watcher) = watcher.take() {
        let _ = active_watcher.stop_and_drain(now_ms()?);
    }
    query_service.begin_shutdown();
    maintenance.begin_shutdown();
    query_service.join();
    maintenance.join();
    let mut stopped = LiveRuntimeStatus::load(&graph)?;
    stopped.state = LiveRuntimeState::Unavailable;
    stopped.store(&graph)?;
    while let Some(reply) = query_service.try_completion() {
        if let Some(stream) = pending.queries.remove(&reply.receipt.request_id) {
            let _ = pending.queue_query(stream, &identity, &reply);
        }
    }
    while let Some(completion) = maintenance.try_completion() {
        if let Some(token) = completion.token
            && let Some(stream) = pending.sync.remove(&token)
        {
            let response = match completion.result {
                Ok(success) => {
                    DaemonResponse::sync(identity.clone(), success.runtime, success.sync)
                }
                Err(error) => DaemonResponse::error(identity.clone(), &error),
            };
            let _ = pending.queue_bulk(stream, &response);
        }
    }
    for (_, stream) in std::mem::take(&mut pending.queries) {
        let _ = pending.queue_bulk(
            stream,
            &DaemonResponse::error(identity.clone(), "atlas-query-unavailable: daemon stopped"),
        );
    }
    for (_, stream) in std::mem::take(&mut pending.sync) {
        let _ = pending.queue_bulk(
            stream,
            &DaemonResponse::error(
                identity.clone(),
                "atlas-maintenance-unavailable: daemon stopped",
            ),
        );
    }
    pending.inbound.clear();
    let io_deadline = Instant::now() + IO_TIMEOUT;
    while (!pending.control_outbound.is_empty() || !pending.bulk_outbound.is_empty())
        && Instant::now() < io_deadline
    {
        pending.drain_outbound();
        std::thread::yield_now();
    }
    remove_registry_if_owned(&graph, &identity);
    drop(watcher);
    drop(daemon_lease);
    Ok(())
}

fn handle_request(
    stream: TcpStream,
    request: DaemonRequest,
    identity: &DaemonIdentity,
    graph: &Path,
    query_service: &mut crate::atlas_query_service::QueryService,
    maintenance: &mut MaintenanceLane,
    pending: &mut PendingResponses,
) -> Result<bool, DaemonError> {
    if request.schema_id != PROTOCOL_SCHEMA
        || request.schema_version != PROTOCOL_VERSION
        || request.identity != *identity
    {
        pending.queue_control(
            stream,
            &DaemonResponse::error(identity.clone(), "daemon identity handshake mismatch"),
        )?;
        return Ok(false);
    }
    if let Err(error) = request.validate_payload() {
        pending.queue_control(
            stream,
            &DaemonResponse::error(identity.clone(), &error.to_string()),
        )?;
        return Ok(false);
    }
    match request.command {
        DaemonCommand::Status => {
            let runtime = LiveRuntimeStatus::load(graph)?;
            pending.queue_control(stream, &DaemonResponse::status(identity.clone(), runtime))?;
            Ok(false)
        }
        DaemonCommand::Sync => {
            if !pending.has_bulk_capacity() {
                pending.queue_control(
                    stream,
                    &DaemonResponse::error(
                        identity.clone(),
                        "atlas-maintenance-busy: daemon pending response capacity is full",
                    ),
                )?;
                return Ok(false);
            }
            let token = pending.next_maintenance_token;
            pending.next_maintenance_token = pending.next_maintenance_token.saturating_add(1);
            match maintenance.try_submit(Some(token)) {
                Ok(()) => {
                    pending.sync.insert(token, stream);
                }
                Err(error) => {
                    pending
                        .queue_control(stream, &DaemonResponse::error(identity.clone(), error))?;
                }
            }
            Ok(false)
        }
        DaemonCommand::Stop => {
            let runtime = LiveRuntimeStatus::load(graph)?;
            query_service.begin_shutdown();
            maintenance.begin_shutdown();
            let _ =
                pending.queue_control(stream, &DaemonResponse::status(identity.clone(), runtime));
            Ok(true)
        }
        DaemonCommand::ServiceStatus => {
            pending.queue_control(
                stream,
                &DaemonResponse::service(identity.clone(), query_service.status()),
            )?;
            Ok(false)
        }
        DaemonCommand::Cancel => {
            let Some(payload) = request.cancel else {
                return Err(DaemonError::Protocol(
                    "validated cancel request omitted payload".to_string(),
                ));
            };
            let cancelled = query_service.cancel(&payload.request_id);
            pending.queue_control(
                stream,
                &DaemonResponse::cancel(identity.clone(), payload.request_id, cancelled),
            )?;
            Ok(false)
        }
        DaemonCommand::Context => {
            let Some(payload) = request.context else {
                return Err(DaemonError::Protocol(
                    "validated context request omitted payload".to_string(),
                ));
            };
            if !pending.has_bulk_capacity() {
                let wire = query_service.reject_before_admission(
                    payload.request_id,
                    crate::atlas_query_service::QueryOutcome::Busy,
                    "atlas-query-busy: daemon pending response capacity is full".to_string(),
                );
                let response = DaemonResponse::query(identity.clone(), wire)?;
                pending.queue_control(stream, &response)?;
                return Ok(false);
            }
            let snapshot = match rust_atlas::pin_context_snapshot(graph) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let outcome = if matches!(
                        &error,
                        rust_atlas::AtlasError::MissingGraph { .. }
                            | rust_atlas::AtlasError::QueryIndexMissing { .. }
                    ) {
                        crate::atlas_query_service::QueryOutcome::Unavailable
                    } else {
                        crate::atlas_query_service::QueryOutcome::Failed
                    };
                    let wire = query_service.reject_before_admission(
                        payload.request_id,
                        outcome,
                        error.to_string(),
                    );
                    let response = DaemonResponse::query(identity.clone(), wire)?;
                    pending.queue_control(stream, &response)?;
                    return Ok(false);
                }
            };
            let request_id = payload.request_id.clone();
            let options = payload.options();
            let query_request = crate::atlas_query_service::QueryServiceRequest {
                request_id: request_id.clone(),
                query: payload.query,
                options,
                snapshot,
            };
            match query_service.try_submit(query_request) {
                Ok(()) => {
                    pending.queries.insert(request_id, stream);
                }
                Err(reply) => pending.queue_query(stream, identity, &reply)?,
            }
            Ok(false)
        }
    }
}

enum InboundPoll {
    Pending,
    Closed,
    Request(Box<DaemonRequest>),
    Error(String),
}

fn poll_inbound(connection: &mut InboundConnection, byte_budget: &mut usize) -> InboundPoll {
    if connection.accepted_at.elapsed() >= IO_TIMEOUT {
        return InboundPoll::Error("daemon request read timeout".to_string());
    }
    if let Some(newline) = connection.bytes.iter().position(|byte| *byte == b'\n') {
        if connection.bytes[newline + 1..]
            .iter()
            .any(|byte| !byte.is_ascii_whitespace())
        {
            return InboundPoll::Error("daemon connection contains multiple requests".to_string());
        }
        return match serde_json::from_slice::<DaemonRequest>(&connection.bytes[..newline]) {
            Ok(request) => InboundPoll::Request(Box::new(request)),
            Err(error) => InboundPoll::Error(format!("invalid daemon message: {error}")),
        };
    }
    if connection.bytes.len() >= MAX_PROTOCOL_BYTES {
        return InboundPoll::Error("daemon request exceeds 1 MiB".to_string());
    }
    if *byte_budget == 0 {
        return InboundPoll::Pending;
    }
    let remaining = MAX_PROTOCOL_BYTES.saturating_sub(connection.bytes.len());
    let read_size = remaining.min(8 * 1024).min(*byte_budget);
    let mut buffer = vec![0_u8; read_size];
    match connection.stream.read(&mut buffer) {
        Ok(0) if connection.bytes.is_empty() => InboundPoll::Closed,
        Ok(0) => InboundPoll::Error("daemon request is unterminated".to_string()),
        Ok(read) => {
            connection.bytes.extend_from_slice(&buffer[..read]);
            *byte_budget = byte_budget.saturating_sub(read);
            if let Some(newline) = connection.bytes.iter().position(|byte| *byte == b'\n') {
                if connection.bytes[newline + 1..]
                    .iter()
                    .any(|byte| !byte.is_ascii_whitespace())
                {
                    return InboundPoll::Error(
                        "daemon connection contains multiple requests".to_string(),
                    );
                }
                match serde_json::from_slice::<DaemonRequest>(&connection.bytes[..newline]) {
                    Ok(request) => InboundPoll::Request(Box::new(request)),
                    Err(error) => InboundPoll::Error(format!("invalid daemon message: {error}")),
                }
            } else if connection.bytes.len() >= MAX_PROTOCOL_BYTES {
                InboundPoll::Error("daemon request exceeds 1 MiB".to_string())
            } else {
                InboundPoll::Pending
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => InboundPoll::Pending,
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => InboundPoll::Pending,
        Err(error) => InboundPoll::Error(format!("daemon request read failed: {error}")),
    }
}

fn drain_outbound_queue(
    queue: &mut VecDeque<OutboundConnection>,
    connection_budget: &mut usize,
    byte_budget: &mut usize,
) {
    let initial = queue.len().min(*connection_budget);
    for _ in 0..initial {
        if *byte_budget == 0 {
            break;
        }
        let Some(mut connection) = queue.pop_front() else {
            break;
        };
        if connection.last_progress_at.elapsed() >= IO_TIMEOUT {
            *connection_budget = connection_budget.saturating_sub(1);
            continue;
        }
        let remaining = connection.bytes.len().saturating_sub(connection.offset);
        let write_size = remaining.min(*byte_budget);
        match connection
            .stream
            .write(&connection.bytes[connection.offset..connection.offset + write_size])
        {
            Ok(0) => {}
            Ok(written) => {
                connection.offset = connection.offset.saturating_add(written);
                connection.last_progress_at = Instant::now();
                *byte_budget = byte_budget.saturating_sub(written);
                if connection.offset < connection.bytes.len() {
                    queue.push_back(connection);
                } else {
                    let _ = connection.stream.shutdown(std::net::Shutdown::Write);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                queue.push_back(connection);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                queue.push_back(connection);
            }
            Err(_) => {}
        }
        *connection_budget = connection_budget.saturating_sub(1);
    }
}

fn maintenance_retry_at(graph: &Path, error: &str) -> Result<Option<Instant>, DaemonError> {
    let status = LiveRuntimeStatus::load(graph)?;
    if status.state == LiveRuntimeState::Degraded {
        return Ok(None);
    }
    let attempts = if error.contains("writer") && error.contains("busy") {
        status.retry.writer_lock_attempts
    } else {
        status.retry.ordinary_attempts
    };
    RetryPolicy::default()
        .delay_for_attempt(attempts)
        .map(|delay| delay.map(|delay| Instant::now() + Duration::from_millis(delay)))
        .map_err(DaemonError::from)
}

impl DaemonResponse {
    fn validate_for(&self, command: DaemonCommand) -> Result<(), DaemonError> {
        if !self.ok {
            if self.error.is_some()
                && self.runtime.is_none()
                && self.sync.is_none()
                && self.query.is_none()
                && self.service.is_none()
                && self.cancel.is_none()
            {
                return Ok(());
            }
            return Err(DaemonError::Protocol(
                "daemon error response has contradictory fields".to_string(),
            ));
        }
        if self.error.is_some() {
            return Err(DaemonError::Protocol(
                "successful daemon response contains an error".to_string(),
            ));
        }
        let valid = match command {
            DaemonCommand::Status | DaemonCommand::Stop => {
                self.runtime.is_some()
                    && self.sync.is_none()
                    && self.query.is_none()
                    && self.service.is_none()
                    && self.cancel.is_none()
            }
            DaemonCommand::Sync => {
                self.runtime.is_some()
                    && self.sync.is_some()
                    && self.query.is_none()
                    && self.service.is_none()
                    && self.cancel.is_none()
            }
            DaemonCommand::Context => {
                self.runtime.is_none()
                    && self.sync.is_none()
                    && self.query.is_some()
                    && self.service.is_none()
                    && self.cancel.is_none()
                    && self
                        .query
                        .as_ref()
                        .is_some_and(|query| query.validate_shape().is_ok())
            }
            DaemonCommand::ServiceStatus => {
                self.runtime.is_none()
                    && self.sync.is_none()
                    && self.query.is_none()
                    && self.service.is_some()
                    && self.cancel.is_none()
            }
            DaemonCommand::Cancel => {
                self.runtime.is_none()
                    && self.sync.is_none()
                    && self.query.is_none()
                    && self.service.is_none()
                    && self.cancel.is_some()
            }
        };
        if valid {
            Ok(())
        } else {
            Err(DaemonError::Protocol(
                "daemon response payload does not match command".to_string(),
            ))
        }
    }

    fn status(identity: DaemonIdentity, runtime: LiveRuntimeStatus) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: true,
            runtime: Some(runtime),
            sync: None,
            query: None,
            service: None,
            cancel: None,
            error: None,
        }
    }

    fn sync(identity: DaemonIdentity, runtime: LiveRuntimeStatus, sync: serde_json::Value) -> Self {
        Self {
            sync: Some(sync),
            ..Self::status(identity, runtime)
        }
    }

    fn query(
        identity: DaemonIdentity,
        query: crate::atlas_query_service::QueryServiceWireReply,
    ) -> Result<Self, DaemonError> {
        query.validate_shape().map_err(|error| {
            DaemonError::Protocol(format!("invalid query service response: {error}"))
        })?;
        Ok(Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: true,
            runtime: None,
            sync: None,
            query: Some(query),
            service: None,
            cancel: None,
            error: None,
        })
    }

    fn service(
        identity: DaemonIdentity,
        service: crate::atlas_query_service::QueryServiceStatus,
    ) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: true,
            runtime: None,
            sync: None,
            query: None,
            service: Some(service),
            cancel: None,
            error: None,
        }
    }

    fn cancel(identity: DaemonIdentity, request_id: String, cancelled: bool) -> Self {
        Self {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity,
            ok: true,
            runtime: None,
            sync: None,
            query: None,
            service: None,
            cancel: Some(DaemonCancelResponse {
                request_id,
                cancelled,
            }),
            error: None,
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
            query: None,
            service: None,
            cancel: None,
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
    match send_to_identity(&identity, code, graph, command) {
        Err(error)
            if command_allows_transient_retry(command) && is_transient_command_error(&error) =>
        {
            std::thread::sleep(Duration::from_millis(10));
            send_to_identity(&identity, code, graph, command)
        }
        result => result,
    }
}

fn command_allows_transient_retry(command: DaemonCommand) -> bool {
    matches!(command, DaemonCommand::Status | DaemonCommand::Stop)
}

fn is_transient_command_error(error: &DaemonError) -> bool {
    matches!(error, DaemonError::Io(_) | DaemonError::Unavailable(_))
        || matches!(error, DaemonError::Protocol(detail) if detail == "daemon closed without a response")
}

fn send_to_identity(
    identity: &DaemonIdentity,
    code: &Path,
    graph: &Path,
    command: DaemonCommand,
) -> Result<DaemonResponse, DaemonError> {
    send_request_to_identity(
        identity,
        code,
        graph,
        DaemonRequest::command(identity.clone(), command),
    )
}

fn send_request_to_identity(
    identity: &DaemonIdentity,
    code: &Path,
    graph: &Path,
    request: DaemonRequest,
) -> Result<DaemonResponse, DaemonError> {
    let command = request.command;
    identity.validate_for(code, graph)?;
    let endpoint = identity
        .endpoint
        .parse::<SocketAddr>()
        .map_err(|error| identity_error(error.to_string()))?;
    let mut stream = TcpStream::connect_timeout(&endpoint, IO_TIMEOUT)
        .map_err(|error| DaemonError::Unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(command_read_timeout(request.command)))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(io_error)?;
    write_json_line(&mut stream, &request)?;
    let response = read_line_json::<DaemonResponse>(&mut stream)?
        .ok_or_else(|| DaemonError::Protocol("daemon closed without a response".to_string()))?;
    if response.schema_id != PROTOCOL_SCHEMA
        || response.schema_version != PROTOCOL_VERSION
        || response.identity != *identity
    {
        return Err(identity_error("daemon response identity mismatch"));
    }
    response.validate_for(command)?;
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
        DaemonCommand::Status
        | DaemonCommand::Stop
        | DaemonCommand::Cancel
        | DaemonCommand::ServiceStatus => IO_TIMEOUT,
        DaemonCommand::Sync | DaemonCommand::Context => SYNC_RESPONSE_TIMEOUT,
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

fn encode_query_reply(
    identity: &DaemonIdentity,
    reply: &crate::atlas_query_service::QueryServiceReply,
) -> Result<Vec<u8>, DaemonError> {
    let wire = crate::atlas_query_service::QueryServiceWireReply::from_reply(reply)
        .map_err(|error| DaemonError::Protocol(error.to_string()))?;
    encode_query_wire(identity, reply, wire)
}

fn encode_query_wire(
    identity: &DaemonIdentity,
    reply: &crate::atlas_query_service::QueryServiceReply,
    wire: crate::atlas_query_service::QueryServiceWireReply,
) -> Result<Vec<u8>, DaemonError> {
    let response = DaemonResponse::query(identity.clone(), wire)?;
    let bytes =
        serde_json::to_vec(&response).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    if bytes.len() >= MAX_PROTOCOL_BYTES {
        let compact = DaemonResponse::query(
            identity.clone(),
            crate::atlas_query_service::QueryServiceWireReply::oversize_failure(reply),
        )?;
        return encode_json_line(&compact);
    }
    encode_json_line(&response)
}

fn encode_json_line<T: Serialize>(value: &T) -> Result<Vec<u8>, DaemonError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    if bytes.len() >= MAX_PROTOCOL_BYTES {
        return Err(DaemonError::Protocol(
            "daemon message exceeds 1 MiB".to_string(),
        ));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_json_line<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let bytes = encode_json_line(value)?;
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
    use std::sync::{Arc, Barrier};
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
                    query_config: crate::atlas_query_service::QueryServiceConfig::default(),
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

    fn spawn_test_server_with_runners(
        code: &Path,
        graph: &Path,
        query_config: crate::atlas_query_service::QueryServiceConfig,
        query_runner: Arc<dyn crate::atlas_query_service::QueryRunner>,
        maintenance_runner: Arc<dyn MaintenanceRunner>,
    ) -> (DaemonIdentity, thread::JoinHandle<()>) {
        let (ready_tx, ready_rx) = mpsc::channel();
        let code = code.to_path_buf();
        let graph = graph.to_path_buf();
        let handle = thread::spawn(move || {
            serve_with_runners(
                &code,
                &graph,
                ServeOptions {
                    auto_sync: false,
                    watch: false,
                    poll_interval: Duration::from_millis(5),
                    query_config,
                },
                Some(ready_tx),
                Some(query_runner),
                Some(maintenance_runner),
            )
            .unwrap();
        });
        (
            ready_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            handle,
        )
    }

    struct BarrierQueryRunner {
        inner: crate::atlas_query_service::PinnedContextRunner,
        started: mpsc::Sender<String>,
        release: Arc<Barrier>,
    }

    impl crate::atlas_query_service::QueryRunner for BarrierQueryRunner {
        fn run(
            &self,
            request: &crate::atlas_query_service::QueryServiceRequest,
            control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            let _ = self.started.send(
                request
                    .snapshot
                    .generation()
                    .unwrap_or_default()
                    .to_string(),
            );
            self.release.wait();
            control.checkpoint()?;
            crate::atlas_query_service::QueryRunner::run(&self.inner, request, control)
        }
    }

    struct BarrierMaintenanceRunner {
        started: mpsc::Sender<()>,
        release: Arc<Barrier>,
    }

    impl MaintenanceRunner for BarrierMaintenanceRunner {
        fn run(
            &self,
            _code: &Path,
            graph: &Path,
            cancellation: Arc<AtomicBool>,
        ) -> Result<MaintenanceSuccess, String> {
            let _ = self.started.send(());
            self.release.wait();
            let mut runtime = LiveRuntimeStatus::load(graph).map_err(|error| error.to_string())?;
            runtime.state = LiveRuntimeState::Healthy;
            runtime.store(graph).map_err(|error| error.to_string())?;
            if cancellation.load(Ordering::Acquire) {
                return Err("atlas-cancelled: maintenance cancelled".to_string());
            }
            Ok(MaintenanceSuccess {
                runtime,
                sync: serde_json::json!({ "fixture": "completed" }),
            })
        }
    }

    fn context_request(identity: &DaemonIdentity, request_id: &str, query: &str) -> DaemonRequest {
        DaemonRequest {
            schema_id: PROTOCOL_SCHEMA.to_string(),
            schema_version: PROTOCOL_VERSION,
            identity: identity.clone(),
            command: DaemonCommand::Context,
            context: Some(DaemonContextRequest {
                request_id: request_id.to_string(),
                query: query.to_string(),
                profile: rust_atlas::ContextProfile::Symbol,
                max_serialized_bytes: None,
                min_score: None,
                after: None,
                expected_graph_fingerprint: None,
                failure_evidence: Vec::new(),
            }),
            cancel: None,
        }
    }

    fn send_context(
        identity: &DaemonIdentity,
        code: &Path,
        graph: &Path,
        request_id: &str,
        query: &str,
    ) -> Result<DaemonResponse, DaemonError> {
        send_request_to_identity(
            identity,
            code,
            graph,
            context_request(identity, request_id, query),
        )
    }

    fn socket_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        server.set_nonblocking(true).unwrap();
        (client, server)
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
        assert!(command_allows_transient_retry(DaemonCommand::Status));
        assert!(command_allows_transient_retry(DaemonCommand::Stop));
        assert!(!command_allows_transient_retry(DaemonCommand::Sync));
    }

    #[test]
    fn test_atlas_daemon_io_reserves_control_and_prevents_bulk_starvation() {
        let (code, graph) = fixture("io-capacity");
        let (_capacity_client, capacity_server) = socket_pair();
        let mut capacity = PendingResponses::new();
        for index in 0..MAX_PENDING_IO {
            capacity.queries.insert(
                format!("reserved-{index}"),
                capacity_server.try_clone().unwrap(),
            );
        }
        assert!(!capacity.has_bulk_capacity());
        assert!(capacity.has_accept_capacity());

        let (mut rejection_client, rejection_server) = socket_pair();
        let identity = DaemonIdentity::new_for_test(
            &code,
            &graph,
            std::process::id(),
            "ca".repeat(16),
            "127.0.0.1:1",
        )
        .unwrap();
        let mut query_service = crate::atlas_query_service::QueryService::new(
            crate::atlas_query_service::QueryServiceConfig::default(),
            Arc::new(crate::atlas_query_service::PinnedContextRunner::new(
                &code, &graph,
            )),
        )
        .unwrap();
        let mut maintenance =
            MaintenanceLane::new(code.clone(), graph.clone(), Arc::new(SyncOnceRunner));
        assert!(
            !handle_request(
                rejection_server,
                context_request(&identity, "capacity-rejection", "value"),
                &identity,
                &graph,
                &mut query_service,
                &mut maintenance,
                &mut capacity,
            )
            .unwrap()
        );
        capacity.drain_outbound();
        let rejection = read_line_json::<DaemonResponse>(&mut rejection_client)
            .unwrap()
            .unwrap()
            .query
            .unwrap();
        assert_eq!(
            rejection.outcome,
            crate::atlas_query_service::QueryOutcome::Busy
        );
        assert_eq!(rejection.retry_after_ms, Some(100));
        assert_eq!(rejection.receipt.request_id, "capacity-rejection");
        assert_eq!(query_service.status().outstanding, 0);
        assert_eq!(query_service.status().busy, 1);

        let (mut sync_client, sync_server) = socket_pair();
        assert!(
            !handle_request(
                sync_server,
                DaemonRequest::command(identity.clone(), DaemonCommand::Sync),
                &identity,
                &graph,
                &mut query_service,
                &mut maintenance,
                &mut capacity,
            )
            .unwrap()
        );
        capacity.drain_outbound();
        let sync_rejection = read_line_json::<DaemonResponse>(&mut sync_client)
            .unwrap()
            .unwrap();
        assert!(
            sync_rejection
                .error
                .unwrap()
                .contains("atlas-maintenance-busy")
        );
        assert!(!maintenance.has_outstanding());

        for _ in 0..CONTROL_IO_RESERVE {
            capacity.inbound.push_back(InboundConnection {
                stream: capacity_server.try_clone().unwrap(),
                bytes: Vec::new(),
                accepted_at: Instant::now(),
            });
        }
        assert!(!capacity.has_accept_capacity());

        let (_control_client, control_server) = socket_pair();
        let (_bulk_client, bulk_server) = socket_pair();
        let mut fairness = PendingResponses::new();
        for _ in 0..IO_CONNECTION_BUDGET {
            fairness.control_outbound.push_back(OutboundConnection {
                stream: control_server.try_clone().unwrap(),
                bytes: b"{}\n".to_vec(),
                offset: 0,
                last_progress_at: Instant::now(),
            });
        }
        fairness.bulk_outbound.push_back(OutboundConnection {
            stream: bulk_server,
            bytes: b"{}\n".to_vec(),
            offset: 0,
            last_progress_at: Instant::now(),
        });
        fairness.drain_outbound();
        assert!(fairness.bulk_outbound.is_empty());
        assert!(!fairness.control_outbound.is_empty());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_protocol_v2_preserves_legacy_shape_and_validates_before_pin() {
        let (code, graph) = fixture("protocol-v2");
        let (identity, handle) = spawn_test_server(&code, &graph);
        let status_response = DaemonResponse::status(
            identity.clone(),
            LiveRuntimeStatus::new(LiveRuntimeState::Warming),
        );
        status_response.validate_for(DaemonCommand::Status).unwrap();
        let status = serde_json::to_value(&status_response).unwrap();
        let object = status.as_object().unwrap();
        assert!(object.contains_key("sync"));
        assert!(object.contains_key("error"));
        assert!(!object.contains_key("query"));
        assert!(!object.contains_key("service"));
        assert!(!object.contains_key("cancel"));
        let mut contradictory = status_response;
        contradictory.sync = Some(serde_json::json!({ "unexpected": true }));
        assert!(contradictory.validate_for(DaemonCommand::Status).is_err());

        let mut invalid = context_request(&identity, "invalid request id", "value");
        let error = send_request_to_identity(&identity, &code, &graph, invalid).unwrap_err();
        assert!(error.to_string().contains("atlas-query-request-id"));

        let unpinned = send_context(&identity, &code, &graph, "valid-unpinned", "value")
            .unwrap()
            .query
            .unwrap();
        assert_eq!(
            unpinned.outcome,
            crate::atlas_query_service::QueryOutcome::Unavailable
        );
        assert_eq!(unpinned.receipt.request_id, "valid-unpinned");
        assert_eq!(unpinned.receipt.attempts, 0);

        invalid = context_request(&identity, "valid-id", "value");
        invalid.command = DaemonCommand::Status;
        let error = send_request_to_identity(&identity, &code, &graph, invalid).unwrap_err();
        assert!(error.to_string().contains("payload does not match"));

        stop_server(&code, &graph, handle);
        fs::remove_dir_all(code.parent().unwrap()).ok();
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

    #[test]
    fn test_atlas_query_service_keeps_control_heartbeat_live_during_slow_query() {
        let (code, graph) = fixture("query-heartbeat");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(Barrier::new(3));
        let runner = Arc::new(BarrierQueryRunner {
            inner: crate::atlas_query_service::PinnedContextRunner::new(&code, &graph),
            started: started_tx,
            release: Arc::clone(&release),
        });
        let config = crate::atlas_query_service::QueryServiceConfig {
            workers: 2,
            queue_capacity: 4,
            ..crate::atlas_query_service::QueryServiceConfig::default()
        };
        let (identity, handle) =
            spawn_test_server_with_runners(&code, &graph, config, runner, Arc::new(SyncOnceRunner));
        let queries = (0..6)
            .map(|index| {
                let query_identity = identity.clone();
                let query_code = code.clone();
                let query_graph = graph.clone();
                thread::spawn(move || {
                    send_context(
                        &query_identity,
                        &query_code,
                        &query_graph,
                        &format!("slow-query-{index}"),
                        "value",
                    )
                })
            })
            .collect::<Vec<_>>();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let admission_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let service = send_to_identity(&identity, &code, &graph, DaemonCommand::ServiceStatus)
                .unwrap()
                .service
                .unwrap();
            if service.active == 2 && service.queued == 4 {
                break;
            }
            assert!(
                Instant::now() < admission_deadline,
                "query burst not admitted"
            );
            thread::yield_now();
        }

        let mut partial = TcpStream::connect(&identity.endpoint).unwrap();
        partial.write_all(b"{\"schema_id\":").unwrap();
        let status = send_to_identity(&identity, &code, &graph, DaemonCommand::Status);
        let service = send_to_identity(&identity, &code, &graph, DaemonCommand::ServiceStatus);
        let stop_identity = identity.clone();
        let stop_code = code.clone();
        let stop_graph = graph.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = stop_tx.send(send_to_identity(
                &stop_identity,
                &stop_code,
                &stop_graph,
                DaemonCommand::Stop,
            ));
        });
        let stop = stop_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release.wait();

        assert!(status.unwrap().runtime.is_some());
        let service = service.unwrap().service.unwrap();
        assert_eq!(service.active, 2);
        assert_eq!(service.queued, 4);
        assert!(stop.unwrap().runtime.is_some());
        let outcomes = queries
            .into_iter()
            .map(|query| query.join().unwrap().unwrap().query.unwrap().outcome)
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    **outcome == crate::atlas_query_service::QueryOutcome::Cancelled
                })
                .count(),
            2
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    **outcome == crate::atlas_query_service::QueryOutcome::Unavailable
                })
                .count(),
            4
        );
        drop(partial);
        handle.join().unwrap();
        assert!(!registry_path(&graph).exists());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_worker_pins_generation_across_writer_publish() {
        let (code, graph) = fixture("query-generation");
        let first = rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let runner = Arc::new(BarrierQueryRunner {
            inner: crate::atlas_query_service::PinnedContextRunner::new(&code, &graph),
            started: started_tx,
            release: Arc::clone(&release),
        });
        let config = crate::atlas_query_service::QueryServiceConfig {
            workers: 1,
            queue_capacity: 1,
            ..crate::atlas_query_service::QueryServiceConfig::default()
        };
        let (identity, handle) =
            spawn_test_server_with_runners(&code, &graph, config, runner, Arc::new(SyncOnceRunner));
        let query_identity = identity.clone();
        let query_code = code.clone();
        let query_graph = graph.clone();
        let query = thread::spawn(move || {
            send_context(
                &query_identity,
                &query_code,
                &query_graph,
                "pin-generation",
                "value",
            )
        });
        let admitted_generation = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(admitted_generation, first.generation);

        fs::write(
            code.join("src/lib.rs"),
            "pub fn value() -> u32 { 2 }\npub fn added() -> bool { true }\n",
        )
        .unwrap();
        let second =
            rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        assert_ne!(second.generation, admitted_generation);
        let first_generation = graph.join("generations").join(&admitted_generation);
        assert!(first_generation.is_dir());
        assert_eq!(
            fs::read_dir(graph.join(".runtime/readers"))
                .unwrap()
                .count(),
            1
        );
        release.wait();

        let response = query.join().unwrap().unwrap().query.unwrap();
        assert_eq!(
            response.outcome,
            crate::atlas_query_service::QueryOutcome::Success
        );
        assert_eq!(response.receipt.generation.unwrap(), admitted_generation);
        assert_eq!(
            fs::read_dir(graph.join(".runtime/readers"))
                .unwrap()
                .count(),
            0
        );
        fs::write(
            code.join("src/lib.rs"),
            "pub fn value() -> u32 { 3 }\npub fn final_value() -> bool { true }\n",
        )
        .unwrap();
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        assert!(!first_generation.exists());
        stop_server(&code, &graph, handle);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_stop_cancels_queued_queries_and_drains_workers() {
        let (code, graph) = fixture("query-stop");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let runner = Arc::new(BarrierQueryRunner {
            inner: crate::atlas_query_service::PinnedContextRunner::new(&code, &graph),
            started: started_tx,
            release: Arc::clone(&release),
        });
        let config = crate::atlas_query_service::QueryServiceConfig {
            workers: 1,
            queue_capacity: 1,
            ..crate::atlas_query_service::QueryServiceConfig::default()
        };
        let (identity, handle) =
            spawn_test_server_with_runners(&code, &graph, config, runner, Arc::new(SyncOnceRunner));
        let spawn_query = |request_id: &'static str| {
            let identity = identity.clone();
            let code = code.clone();
            let graph = graph.clone();
            thread::spawn(move || send_context(&identity, &code, &graph, request_id, "value"))
        };
        let running = spawn_query("running-request");
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let queued = spawn_query("queued-request");

        let admission_deadline = Instant::now() + Duration::from_secs(2);
        let service = loop {
            let response =
                send_to_identity(&identity, &code, &graph, DaemonCommand::ServiceStatus).unwrap();
            let service = response.service.unwrap();
            if service.outstanding == 2 {
                break service;
            }
            assert!(
                Instant::now() < admission_deadline,
                "queued request was not admitted"
            );
            thread::yield_now();
        };
        assert_eq!(service.active, 1);
        assert_eq!(service.queued, 1);

        let stop_identity = identity.clone();
        let stop_code = code.clone();
        let stop_graph = graph.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = stop_tx.send(send_to_identity(
                &stop_identity,
                &stop_code,
                &stop_graph,
                DaemonCommand::Stop,
            ));
        });
        let stop_response = stop_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release.wait();
        assert!(stop_response.unwrap().runtime.is_some());

        let running = running.join().unwrap().unwrap().query.unwrap();
        let queued = queued.join().unwrap().unwrap().query.unwrap();
        assert_eq!(
            running.outcome,
            crate::atlas_query_service::QueryOutcome::Cancelled
        );
        assert_eq!(
            queued.outcome,
            crate::atlas_query_service::QueryOutcome::Unavailable
        );
        handle.join().unwrap();
        assert!(!registry_path(&graph).exists());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_daemon_maintenance_lane_keeps_status_live_during_sync() {
        let (code, graph) = fixture("maintenance-heartbeat");
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let maintenance = Arc::new(BarrierMaintenanceRunner {
            started: started_tx,
            release: Arc::clone(&release),
        });
        let (identity, handle) = spawn_test_server_with_runners(
            &code,
            &graph,
            crate::atlas_query_service::QueryServiceConfig::default(),
            Arc::new(crate::atlas_query_service::PinnedContextRunner::new(
                &code, &graph,
            )),
            maintenance,
        );
        let sync_identity = identity.clone();
        let sync_code = code.clone();
        let sync_graph = graph.clone();
        let sync = thread::spawn(move || {
            send_to_identity(&sync_identity, &sync_code, &sync_graph, DaemonCommand::Sync)
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let journal = PendingJournal::open(&graph).unwrap();
        journal
            .record("src/mid-sync.rs", now_ms().unwrap())
            .unwrap();

        let status = send_to_identity(&identity, &code, &graph, DaemonCommand::Status);
        let stop_identity = identity.clone();
        let stop_code = code.clone();
        let stop_graph = graph.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = stop_tx.send(send_to_identity(
                &stop_identity,
                &stop_code,
                &stop_graph,
                DaemonCommand::Stop,
            ));
        });
        let stop = stop_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release.wait();
        assert!(status.unwrap().runtime.is_some());
        assert!(stop.unwrap().runtime.is_some());
        assert!(
            sync.join()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("atlas-cancelled")
        );
        handle.join().unwrap();
        assert!(!registry_path(&graph).exists());
        assert_eq!(
            LiveRuntimeStatus::load(&graph).unwrap().state,
            LiveRuntimeState::Unavailable
        );
        assert!(
            journal
                .snapshot()
                .unwrap()
                .events
                .iter()
                .any(|event| event.path == "src/mid-sync.rs")
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_service_rejects_oversize_protocol_response() {
        let (code, graph) = fixture("oversize-query-response");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        let mut service = crate::atlas_query_service::QueryService::new(
            crate::atlas_query_service::QueryServiceConfig {
                workers: 1,
                queue_capacity: 1,
                ..crate::atlas_query_service::QueryServiceConfig::default()
            },
            Arc::new(crate::atlas_query_service::PinnedContextRunner::new(
                &code, &graph,
            )),
        )
        .unwrap();
        service
            .try_submit(crate::atlas_query_service::QueryServiceRequest {
                request_id: "oversize-response".to_string(),
                query: "value".to_string(),
                options: rust_atlas::ContextOptions {
                    frozen: true,
                    ..rust_atlas::ContextOptions::default()
                },
                snapshot: rust_atlas::pin_context_snapshot(&graph).unwrap(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let reply = loop {
            if let Some(reply) = service.try_completion() {
                break reply;
            }
            assert!(Instant::now() < deadline, "query completion deadlocked");
            thread::yield_now();
        };
        let readers = graph.join(".runtime/readers");
        assert_eq!(fs::read_dir(&readers).unwrap().count(), 1);
        let mut wire =
            crate::atlas_query_service::QueryServiceWireReply::from_reply(&reply).unwrap();
        wire.context = Some(serde_json::json!({
            "oversize": "x".repeat(MAX_PROTOCOL_BYTES)
        }));

        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let identity = DaemonIdentity::new_for_test(
            &code,
            &graph,
            std::process::id(),
            "11".repeat(16),
            &listener.local_addr().unwrap().to_string(),
        )
        .unwrap();
        let bytes = encode_query_wire(&identity, &reply, wire).unwrap();
        server.write_all(&bytes).unwrap();
        server.shutdown(std::net::Shutdown::Write).unwrap();
        let response = read_line_json::<DaemonResponse>(&mut client)
            .unwrap()
            .unwrap();
        assert!(response.ok);
        let query = response.query.unwrap();
        assert_eq!(
            query.outcome,
            crate::atlas_query_service::QueryOutcome::Failed
        );
        assert!(
            query
                .diagnostic
                .unwrap()
                .contains("atlas-query-response-too-large")
        );
        assert!(query.context.is_none());
        assert!(response.error.is_none());
        assert!(
            read_line_json::<DaemonResponse>(&mut client)
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read_dir(&readers).unwrap().count(), 1);
        drop(reply);
        service.shutdown();
        assert_eq!(fs::read_dir(&readers).unwrap().count(), 0);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }
}
