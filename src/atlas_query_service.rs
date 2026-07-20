use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const RECEIPT_SCHEMA: &str = "agent-spec/atlas-query-service/receipt-v1";
const RESPONSE_SCHEMA: &str = "agent-spec/atlas-query-service-response-v1";
const INDEX_MEMORY_MULTIPLIER: u64 = 4;
const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QueryOutcome {
    Success,
    Busy,
    Timeout,
    Cancelled,
    Degraded,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct QueryServiceConfig {
    pub workers: usize,
    pub queue_capacity: usize,
    pub queue_timeout_ms: u64,
    pub deadline_ms: u64,
    pub memory_budget_bytes: u64,
    pub retry_after_ms: u64,
}

impl Default for QueryServiceConfig {
    fn default() -> Self {
        Self {
            workers: 0,
            queue_capacity: 4,
            queue_timeout_ms: 2_000,
            deadline_ms: 20_000,
            memory_budget_bytes: 268_435_456,
            retry_after_ms: 100,
        }
    }
}

impl QueryServiceConfig {
    pub(crate) fn worker_profile() -> Self {
        Self {
            workers: 2,
            ..Self::default()
        }
    }

    pub(crate) fn validate(self) -> Result<(), QueryServiceConfigError> {
        validate_range("workers", self.workers as u64, 0, 4)?;
        if self.workers > 0 {
            validate_range("queue_capacity", self.queue_capacity as u64, 1, 64)?;
        }
        validate_range("queue_timeout_ms", self.queue_timeout_ms, 10, 30_000)?;
        validate_range("deadline_ms", self.deadline_ms, 100, 60_000)?;
        validate_range(
            "memory_budget_bytes",
            self.memory_budget_bytes,
            16_777_216,
            2_147_483_648,
        )?;
        validate_range("retry_after_ms", self.retry_after_ms, 1, 10_000)?;
        Ok(())
    }
}

fn validate_range(
    field: &'static str,
    value: u64,
    minimum: u64,
    maximum: u64,
) -> Result<(), QueryServiceConfigError> {
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(QueryServiceConfigError {
            field,
            value,
            minimum,
            maximum,
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("atlas-query-config: {field}={value} is outside {minimum}..={maximum}")]
pub(crate) struct QueryServiceConfigError {
    pub field: &'static str,
    pub value: u64,
    pub minimum: u64,
    pub maximum: u64,
}

pub(crate) struct QueryServiceRequest {
    pub request_id: String,
    pub query: String,
    pub options: rust_atlas::ContextOptions,
    pub snapshot: rust_atlas::PinnedContextSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QueryServingMode {
    Worker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QueryServiceLimits {
    pub workers: usize,
    pub queue_capacity: usize,
    pub queue_timeout_ms: u64,
    pub deadline_ms: u64,
    pub memory_budget_bytes: u64,
    pub retry_after_ms: u64,
}

impl From<QueryServiceConfig> for QueryServiceLimits {
    fn from(config: QueryServiceConfig) -> Self {
        Self {
            workers: config.workers,
            queue_capacity: config.queue_capacity,
            queue_timeout_ms: config.queue_timeout_ms,
            deadline_ms: config.deadline_ms,
            memory_budget_bytes: config.memory_budget_bytes,
            retry_after_ms: config.retry_after_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueryServiceReceipt {
    pub schema: String,
    pub request_id: String,
    pub serving_mode: QueryServingMode,
    pub limits: QueryServiceLimits,
    pub reservation_bytes: u64,
    pub enqueued_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: u64,
    pub queue_wait_ms: Option<u64>,
    pub execution_ms: Option<u64>,
    pub attempts: u8,
    pub outcome: QueryOutcome,
    pub generation: Option<String>,
    pub graph_fingerprint: String,
    pub load_profile: Option<rust_atlas::QueryLoadProfile>,
    pub response_digest: Option<String>,
    pub fallback_used: bool,
    pub fallback_generation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct QueryServiceReply {
    pub outcome: QueryOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<rust_atlas::ContextResult>,
    pub receipt: QueryServiceReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    #[serde(skip)]
    resources: Option<Arc<QueryReplyResources>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueryServiceWireReply {
    pub schema: String,
    pub outcome: QueryOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    pub receipt: QueryServiceReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl QueryServiceWireReply {
    pub(crate) fn from_reply(reply: &QueryServiceReply) -> Result<Self, serde_json::Error> {
        Ok(Self {
            schema: RESPONSE_SCHEMA.to_string(),
            outcome: reply.outcome,
            context: reply
                .context
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
            receipt: reply.receipt.clone(),
            diagnostic: reply.diagnostic.clone(),
            retry_after_ms: reply.retry_after_ms,
        })
    }

    pub(crate) fn oversize_failure(reply: &QueryServiceReply) -> Self {
        let mut receipt = reply.receipt.clone();
        receipt.outcome = QueryOutcome::Failed;
        receipt.response_digest = None;
        Self {
            schema: RESPONSE_SCHEMA.to_string(),
            outcome: QueryOutcome::Failed,
            context: None,
            receipt,
            diagnostic: Some(
                "atlas-query-response-too-large: finalized response exceeds 1 MiB".to_string(),
            ),
            retry_after_ms: None,
        }
    }

    pub(crate) fn validate_shape(&self) -> Result<(), &'static str> {
        if self.schema != RESPONSE_SCHEMA {
            return Err("query response schema is unsupported");
        }
        if self.receipt.schema != RECEIPT_SCHEMA {
            return Err("query receipt schema is unsupported");
        }
        if self.outcome != self.receipt.outcome {
            return Err("query wire reply and receipt outcomes differ");
        }
        match self.outcome {
            QueryOutcome::Success
                if self.context.is_some()
                    && self.diagnostic.is_none()
                    && self.retry_after_ms.is_none() =>
            {
                Ok(())
            }
            QueryOutcome::Busy | QueryOutcome::Degraded
                if self.context.is_none()
                    && self.diagnostic.is_some()
                    && self.retry_after_ms.is_some() =>
            {
                Ok(())
            }
            QueryOutcome::Timeout
            | QueryOutcome::Cancelled
            | QueryOutcome::Failed
            | QueryOutcome::Unavailable
                if self.context.is_none()
                    && self.diagnostic.is_some()
                    && self.retry_after_ms.is_none() =>
            {
                Ok(())
            }
            _ => Err("query wire outcome fields are not mutually exclusive"),
        }
    }
}

impl QueryServiceReply {
    pub(crate) fn validate_shape(&self) -> Result<(), &'static str> {
        if self.outcome != self.receipt.outcome {
            return Err("query reply and receipt outcomes differ");
        }
        match self.outcome {
            QueryOutcome::Success
                if self.context.is_some()
                    && self.diagnostic.is_none()
                    && self.retry_after_ms.is_none() =>
            {
                Ok(())
            }
            QueryOutcome::Busy | QueryOutcome::Degraded
                if self.context.is_none()
                    && self.diagnostic.is_some()
                    && self.retry_after_ms.is_some() =>
            {
                Ok(())
            }
            QueryOutcome::Timeout
            | QueryOutcome::Cancelled
            | QueryOutcome::Failed
            | QueryOutcome::Unavailable
                if self.context.is_none()
                    && self.diagnostic.is_some()
                    && self.retry_after_ms.is_none() =>
            {
                Ok(())
            }
            _ => Err("query outcome fields are not mutually exclusive"),
        }
    }
}

pub(crate) trait QueryRunner: Send + Sync + 'static {
    fn run(
        &self,
        request: &QueryServiceRequest,
        control: &rust_atlas::ContextExecutionControl,
    ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError>;
}

pub(crate) struct PinnedContextRunner {
    code_root: PathBuf,
    graph_dir: PathBuf,
}

impl PinnedContextRunner {
    pub(crate) fn new(code_root: &Path, graph_dir: &Path) -> Self {
        Self {
            code_root: code_root.to_path_buf(),
            graph_dir: graph_dir.to_path_buf(),
        }
    }
}

impl QueryRunner for PinnedContextRunner {
    fn run(
        &self,
        request: &QueryServiceRequest,
        control: &rust_atlas::ContextExecutionControl,
    ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
        let intent = rust_atlas::parse_query_intent(&request.query, request.options.profile);
        let retrieval = rust_atlas::retrieve_context_pinned(
            &self.code_root,
            &self.graph_dir,
            &request.snapshot,
            &intent,
            &request.options,
            control,
        )?;
        rust_atlas::project_context_controlled(
            &self.code_root,
            &retrieval,
            &request.options,
            control,
        )
    }
}

pub(crate) trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> u64;
}

struct MonotonicClock {
    origin: Instant,
}

impl MonotonicClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QueryServiceStatus {
    pub enabled: bool,
    pub workers: usize,
    pub queue_capacity: usize,
    pub queued: usize,
    pub active: usize,
    pub outstanding: usize,
    pub reserved_bytes: u64,
    pub memory_budget_bytes: u64,
    pub circuit_open: bool,
    pub shutting_down: bool,
    pub accepted: u64,
    pub started: u64,
    pub completed: u64,
    pub busy: u64,
    pub timed_out: u64,
    pub cancelled: u64,
    pub degraded: u64,
    pub failed: u64,
    pub unavailable: u64,
    pub panics: u64,
}

struct ServiceState {
    shutting_down: bool,
    circuit_open: bool,
    queued: usize,
    active: usize,
    outstanding: usize,
    reserved_bytes: u64,
    request_ids: BTreeSet<String>,
    controls: BTreeMap<String, rust_atlas::ContextExecutionControl>,
    accepted: u64,
    started: u64,
    completed: u64,
    busy: u64,
    timed_out: u64,
    cancelled: u64,
    degraded: u64,
    failed: u64,
    unavailable: u64,
    panics: u64,
}

struct QueryReplyResources {
    _snapshot: rust_atlas::PinnedContextSnapshot,
    state: Arc<Mutex<ServiceState>>,
    request_id: String,
    reservation_bytes: u64,
}

impl std::fmt::Debug for QueryReplyResources {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QueryReplyResources")
            .field("request_id", &self.request_id)
            .field("reservation_bytes", &self.reservation_bytes)
            .finish_non_exhaustive()
    }
}

impl Drop for QueryReplyResources {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        state.outstanding = state.outstanding.saturating_sub(1);
        state.reserved_bytes = state.reserved_bytes.saturating_sub(self.reservation_bytes);
        state.request_ids.remove(&self.request_id);
    }
}

impl ServiceState {
    fn new() -> Self {
        Self {
            shutting_down: false,
            circuit_open: false,
            queued: 0,
            active: 0,
            outstanding: 0,
            reserved_bytes: 0,
            request_ids: BTreeSet::new(),
            controls: BTreeMap::new(),
            accepted: 0,
            started: 0,
            completed: 0,
            busy: 0,
            timed_out: 0,
            cancelled: 0,
            degraded: 0,
            failed: 0,
            unavailable: 0,
            panics: 0,
        }
    }
}

struct QueryJob {
    request: QueryServiceRequest,
    control: rust_atlas::ContextExecutionControl,
    reservation_bytes: u64,
    enqueued_at_ms: u64,
}

struct CompletedJob {
    reply: QueryServiceReply,
}

pub(crate) struct QueryService {
    config: QueryServiceConfig,
    clock: Arc<dyn Clock>,
    state: Arc<Mutex<ServiceState>>,
    sender: Option<SyncSender<QueryJob>>,
    completions: Receiver<CompletedJob>,
    workers: Vec<JoinHandle<()>>,
}

impl QueryService {
    pub(crate) fn new(
        config: QueryServiceConfig,
        runner: Arc<dyn QueryRunner>,
    ) -> Result<Self, QueryServiceConfigError> {
        Self::with_clock(config, runner, Arc::new(MonotonicClock::new()))
    }

    pub(crate) fn with_clock(
        config: QueryServiceConfig,
        runner: Arc<dyn QueryRunner>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, QueryServiceConfigError> {
        config.validate()?;
        let state = Arc::new(Mutex::new(ServiceState::new()));
        let queue_capacity = if config.workers == 0 {
            1
        } else {
            config.queue_capacity
        };
        let completion_capacity = config.workers.saturating_add(queue_capacity).max(1);
        let (sender, receiver) = sync_channel::<QueryJob>(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let (completion_sender, completions) = sync_channel(completion_capacity);
        let mut workers = Vec::with_capacity(config.workers);
        for position in 0..config.workers {
            let worker_receiver = Arc::clone(&receiver);
            let worker_completions = completion_sender.clone();
            let worker_state = Arc::clone(&state);
            let worker_runner = Arc::clone(&runner);
            let worker_clock = Arc::clone(&clock);
            let name = format!("atlas-query-{position}");
            if let Ok(handle) = std::thread::Builder::new().name(name).spawn(move || {
                worker_loop(
                    config,
                    worker_receiver,
                    worker_completions,
                    worker_state,
                    worker_runner,
                    worker_clock,
                );
            }) {
                workers.push(handle);
            }
        }
        drop(completion_sender);
        Ok(Self {
            config,
            clock,
            state,
            sender: (config.workers > 0 && workers.len() == config.workers).then_some(sender),
            completions,
            workers,
        })
    }

    pub(crate) fn try_submit(
        &self,
        request: QueryServiceRequest,
    ) -> Result<(), Box<QueryServiceReply>> {
        let now_ms = self.clock.now_ms();
        if !valid_request_id(&request.request_id) {
            let mut state = lock(&self.state);
            state.failed = state.failed.saturating_add(1);
            return Err(rejected_reply(
                self.config,
                &request,
                now_ms,
                0,
                QueryOutcome::Failed,
                "atlas-query-request-id: expected 1..=64 ASCII alphanumeric, '-' or '_'",
            ));
        }
        if !request.options.frozen {
            let mut state = lock(&self.state);
            state.failed = state.failed.saturating_add(1);
            return Err(rejected_reply(
                self.config,
                &request,
                now_ms,
                0,
                QueryOutcome::Failed,
                "atlas-query-not-frozen: worker queries require frozen=true",
            ));
        }
        let output_limit =
            match rust_atlas::evidence_priority_plan(request.options.profile, &request.options) {
                Ok(plan) => plan.limits.max_serialized_bytes,
                Err(error) => {
                    let mut state = lock(&self.state);
                    state.failed = state.failed.saturating_add(1);
                    return Err(rejected_reply(
                        self.config,
                        &request,
                        now_ms,
                        0,
                        QueryOutcome::Failed,
                        &error.to_string(),
                    ));
                }
            };
        let reservation_bytes = match reservation_bytes(&request, output_limit) {
            Ok(value) => value,
            Err(diagnostic) => {
                let mut state = lock(&self.state);
                state.busy = state.busy.saturating_add(1);
                return Err(rejected_reply(
                    self.config,
                    &request,
                    now_ms,
                    u64::MAX,
                    QueryOutcome::Busy,
                    diagnostic,
                ));
            }
        };
        let Some(sender) = self.sender.as_ref() else {
            let mut state = lock(&self.state);
            state.unavailable = state.unavailable.saturating_add(1);
            return Err(rejected_reply(
                self.config,
                &request,
                now_ms,
                reservation_bytes,
                QueryOutcome::Unavailable,
                "atlas-query-unavailable: worker serving is disabled",
            ));
        };

        let control = rust_atlas::ContextExecutionControl::unlimited();
        {
            let mut state = lock(&self.state);
            if state.shutting_down {
                state.unavailable = state.unavailable.saturating_add(1);
                return Err(rejected_reply(
                    self.config,
                    &request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Unavailable,
                    "atlas-query-unavailable: service is shutting down",
                ));
            }
            if state.circuit_open {
                state.degraded = state.degraded.saturating_add(1);
                return Err(rejected_reply(
                    self.config,
                    &request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Degraded,
                    "atlas-query-degraded: worker panic circuit is open",
                ));
            }
            if state.request_ids.contains(&request.request_id) {
                state.failed = state.failed.saturating_add(1);
                return Err(rejected_reply(
                    self.config,
                    &request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Failed,
                    "atlas-query-duplicate-id: request id is already outstanding",
                ));
            }
            let max_outstanding = self
                .config
                .workers
                .saturating_add(self.config.queue_capacity);
            let memory_after = state.reserved_bytes.checked_add(reservation_bytes);
            if state.outstanding >= max_outstanding
                || memory_after.is_none_or(|value| value > self.config.memory_budget_bytes)
            {
                state.busy = state.busy.saturating_add(1);
                return Err(rejected_reply(
                    self.config,
                    &request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Busy,
                    "atlas-query-busy: queue or memory reservation is full",
                ));
            }
            state.reserved_bytes = memory_after.unwrap_or(state.reserved_bytes);
            state.outstanding += 1;
            state.queued += 1;
            state.accepted = state.accepted.saturating_add(1);
            state.request_ids.insert(request.request_id.clone());
            state
                .controls
                .insert(request.request_id.clone(), control.clone());
        }

        let job = QueryJob {
            request,
            control,
            reservation_bytes,
            enqueued_at_ms: now_ms,
        };
        match sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(job)) => {
                self.rollback_admission(&job);
                let mut state = lock(&self.state);
                state.busy = state.busy.saturating_add(1);
                Err(rejected_reply(
                    self.config,
                    &job.request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Busy,
                    "atlas-query-busy: bounded queue is full",
                ))
            }
            Err(TrySendError::Disconnected(job)) => {
                self.rollback_admission(&job);
                let mut state = lock(&self.state);
                state.unavailable = state.unavailable.saturating_add(1);
                Err(rejected_reply(
                    self.config,
                    &job.request,
                    now_ms,
                    reservation_bytes,
                    QueryOutcome::Unavailable,
                    "atlas-query-unavailable: worker queue is disconnected",
                ))
            }
        }
    }

    fn rollback_admission(&self, job: &QueryJob) {
        let mut state = lock(&self.state);
        state.outstanding = state.outstanding.saturating_sub(1);
        state.queued = state.queued.saturating_sub(1);
        state.reserved_bytes = state.reserved_bytes.saturating_sub(job.reservation_bytes);
        state.request_ids.remove(&job.request.request_id);
        state.controls.remove(&job.request.request_id);
    }

    pub(crate) fn cancel(&self, request_id: &str) -> bool {
        let state = lock(&self.state);
        let Some(control) = state.controls.get(request_id) else {
            return false;
        };
        control.cancel();
        true
    }

    pub(crate) fn try_completion(&self) -> Option<QueryServiceReply> {
        match self.completions.try_recv() {
            Ok(completed) => Some(completed.reply),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub(crate) fn status(&self) -> QueryServiceStatus {
        let state = lock(&self.state);
        QueryServiceStatus {
            enabled: self.sender.is_some(),
            workers: self.config.workers,
            queue_capacity: self.config.queue_capacity,
            queued: state.queued,
            active: state.active,
            outstanding: state.outstanding,
            reserved_bytes: state.reserved_bytes,
            memory_budget_bytes: self.config.memory_budget_bytes,
            circuit_open: state.circuit_open,
            shutting_down: state.shutting_down,
            accepted: state.accepted,
            started: state.started,
            completed: state.completed,
            busy: state.busy,
            timed_out: state.timed_out,
            cancelled: state.cancelled,
            degraded: state.degraded,
            failed: state.failed,
            unavailable: state.unavailable,
            panics: state.panics,
        }
    }

    pub(crate) fn reject_before_admission(
        &self,
        request_id: String,
        outcome: QueryOutcome,
        diagnostic: String,
    ) -> QueryServiceWireReply {
        let now_ms = self.clock.now_ms();
        let mut state = lock(&self.state);
        match outcome {
            QueryOutcome::Busy => state.busy = state.busy.saturating_add(1),
            QueryOutcome::Timeout => state.timed_out = state.timed_out.saturating_add(1),
            QueryOutcome::Cancelled => state.cancelled = state.cancelled.saturating_add(1),
            QueryOutcome::Degraded => state.degraded = state.degraded.saturating_add(1),
            QueryOutcome::Failed | QueryOutcome::Success => {
                state.failed = state.failed.saturating_add(1)
            }
            QueryOutcome::Unavailable => state.unavailable = state.unavailable.saturating_add(1),
        }
        let retry_after_ms = matches!(outcome, QueryOutcome::Busy | QueryOutcome::Degraded)
            .then_some(self.config.retry_after_ms);
        QueryServiceWireReply {
            schema: RESPONSE_SCHEMA.to_string(),
            outcome,
            context: None,
            receipt: QueryServiceReceipt {
                schema: RECEIPT_SCHEMA.to_string(),
                request_id,
                serving_mode: QueryServingMode::Worker,
                limits: self.config.into(),
                reservation_bytes: 0,
                enqueued_at_ms: now_ms,
                started_at_ms: None,
                completed_at_ms: now_ms,
                queue_wait_ms: None,
                execution_ms: None,
                attempts: 0,
                outcome,
                generation: None,
                graph_fingerprint: String::new(),
                load_profile: None,
                response_digest: None,
                fallback_used: false,
                fallback_generation: None,
            },
            diagnostic: Some(diagnostic),
            retry_after_ms,
        }
    }

    pub(crate) fn begin_shutdown(&mut self) {
        {
            let mut state = lock(&self.state);
            state.shutting_down = true;
            for control in state.controls.values() {
                control.cancel();
            }
        }
        self.sender.take();
    }

    pub(crate) fn join(&mut self) {
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }

    pub(crate) fn shutdown(&mut self) {
        self.begin_shutdown();
        self.join();
    }
}

impl Drop for QueryService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_loop(
    config: QueryServiceConfig,
    receiver: Arc<Mutex<Receiver<QueryJob>>>,
    completions: SyncSender<CompletedJob>,
    state: Arc<Mutex<ServiceState>>,
    runner: Arc<dyn QueryRunner>,
    clock: Arc<dyn Clock>,
) {
    loop {
        let received = {
            let receiver = lock(&receiver);
            receiver.recv()
        };
        let Ok(job) = received else {
            break;
        };
        run_job(config, job, &completions, &state, &runner, &clock);
    }
}

fn run_job(
    config: QueryServiceConfig,
    job: QueryJob,
    completions: &SyncSender<CompletedJob>,
    state: &Arc<Mutex<ServiceState>>,
    runner: &Arc<dyn QueryRunner>,
    clock: &Arc<dyn Clock>,
) {
    let started_at_ms = clock.now_ms();
    let queue_wait_ms = started_at_ms.saturating_sub(job.enqueued_at_ms);
    let execution_control = rust_atlas::ContextExecutionControl::with_deadline(
        Instant::now() + Duration::from_millis(config.deadline_ms),
    );
    let early_outcome = {
        let mut service = lock(state);
        service.queued = service.queued.saturating_sub(1);
        if service.shutting_down {
            Some((
                QueryOutcome::Unavailable,
                "atlas-query-unavailable: service stopped before execution",
            ))
        } else if job.control.checkpoint().is_err() {
            Some((
                QueryOutcome::Cancelled,
                "atlas-query-cancelled: cancelled before execution",
            ))
        } else if queue_wait_ms >= config.queue_timeout_ms {
            Some((
                QueryOutcome::Timeout,
                "atlas-query-timeout: queue deadline elapsed",
            ))
        } else {
            service.active += 1;
            service.started = service.started.saturating_add(1);
            service
                .controls
                .insert(job.request.request_id.clone(), execution_control.clone());
            None
        }
    };
    if let Some((outcome, diagnostic)) = early_outcome {
        finish_job(
            config,
            job,
            completions,
            state,
            clock,
            None,
            None,
            0,
            outcome,
            Some(diagnostic.into()),
        );
        return;
    }

    let mut attempts = 0_u8;
    let (context, outcome, diagnostic) = loop {
        attempts = attempts.saturating_add(1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner.run(&job.request, &execution_control)
        }));
        match result {
            Ok(Ok(context)) => break (Some(context), QueryOutcome::Success, None),
            Ok(Err(rust_atlas::AtlasError::QueryCancelled)) => {
                break (
                    None,
                    QueryOutcome::Cancelled,
                    Some("atlas-query-cancelled: runner observed cancellation".into()),
                );
            }
            Ok(Err(rust_atlas::AtlasError::QueryTimeout)) => {
                break (
                    None,
                    QueryOutcome::Timeout,
                    Some("atlas-query-timeout: execution deadline elapsed".into()),
                );
            }
            Ok(Err(error)) => break (None, QueryOutcome::Failed, Some(error.to_string())),
            Err(_) => {
                let mut service = lock(state);
                service.panics = service.panics.saturating_add(1);
                if attempts >= 2 {
                    service.circuit_open = true;
                    break (
                        None,
                        QueryOutcome::Degraded,
                        Some("atlas-query-degraded: worker panicked twice; circuit opened".into()),
                    );
                }
            }
        }
    };
    finish_job(
        config,
        job,
        completions,
        state,
        clock,
        Some(started_at_ms),
        context,
        attempts,
        outcome,
        diagnostic,
    );
}

#[allow(clippy::too_many_arguments)]
fn finish_job(
    config: QueryServiceConfig,
    job: QueryJob,
    completions: &SyncSender<CompletedJob>,
    state: &Arc<Mutex<ServiceState>>,
    clock: &Arc<dyn Clock>,
    started_at_ms: Option<u64>,
    context: Option<rust_atlas::ContextResult>,
    attempts: u8,
    outcome: QueryOutcome,
    diagnostic: Option<String>,
) {
    let completed_at_ms = clock.now_ms();
    let response_digest = context.as_ref().and_then(|value| {
        serde_json::to_vec(value)
            .ok()
            .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
    });
    let load_profile = context.as_ref().map(|value| value.receipt.load_profile);
    let generation = job.request.snapshot.generation().map(str::to_string);
    let graph_fingerprint = job.request.snapshot.graph_fingerprint().to_string();
    let request_id = job.request.request_id.clone();
    let reservation_bytes = job.reservation_bytes;
    let queue_wait_ms = started_at_ms.map(|started| started.saturating_sub(job.enqueued_at_ms));
    let execution_ms = started_at_ms.map(|started| completed_at_ms.saturating_sub(started));
    let retry_after_ms = matches!(outcome, QueryOutcome::Busy | QueryOutcome::Degraded)
        .then_some(config.retry_after_ms);
    let receipt = QueryServiceReceipt {
        schema: RECEIPT_SCHEMA.into(),
        request_id: request_id.clone(),
        serving_mode: QueryServingMode::Worker,
        limits: config.into(),
        reservation_bytes: job.reservation_bytes,
        enqueued_at_ms: job.enqueued_at_ms,
        started_at_ms,
        completed_at_ms,
        queue_wait_ms,
        execution_ms,
        attempts,
        outcome,
        generation,
        graph_fingerprint,
        load_profile,
        response_digest,
        fallback_used: false,
        fallback_generation: None,
    };
    let reply = QueryServiceReply {
        outcome,
        context,
        receipt,
        diagnostic,
        retry_after_ms,
        resources: Some(Arc::new(QueryReplyResources {
            _snapshot: job.request.snapshot,
            state: Arc::clone(state),
            request_id: request_id.clone(),
            reservation_bytes,
        })),
    };
    {
        let mut service = lock(state);
        if started_at_ms.is_some() {
            service.active = service.active.saturating_sub(1);
        }
        service.controls.remove(&request_id);
        service.completed = service.completed.saturating_add(1);
        match outcome {
            QueryOutcome::Success => {}
            QueryOutcome::Busy => service.busy = service.busy.saturating_add(1),
            QueryOutcome::Timeout => service.timed_out = service.timed_out.saturating_add(1),
            QueryOutcome::Cancelled => service.cancelled = service.cancelled.saturating_add(1),
            QueryOutcome::Degraded => service.degraded = service.degraded.saturating_add(1),
            QueryOutcome::Failed => service.failed = service.failed.saturating_add(1),
            QueryOutcome::Unavailable => {
                service.unavailable = service.unavailable.saturating_add(1)
            }
        }
    }
    let _ = completions.send(CompletedJob { reply });
}

fn reservation_bytes(
    request: &QueryServiceRequest,
    max_serialized_bytes: usize,
) -> Result<u64, &'static str> {
    let request_bytes = request
        .request_id
        .len()
        .checked_add(request.query.len())
        .and_then(|value| {
            request
                .options
                .after
                .iter()
                .chain(request.options.expected_graph_fingerprint.iter())
                .chain(request.options.failure_evidence.iter())
                .try_fold(value, |total, item| total.checked_add(item.len()))
        })
        .ok_or("atlas-query-busy: request size overflow")?;
    request
        .snapshot
        .estimated_index_bytes()
        .checked_mul(INDEX_MEMORY_MULTIPLIER)
        .and_then(|value| value.checked_add(u64::try_from(request_bytes).ok()?))
        .and_then(|value| value.checked_add(u64::try_from(max_serialized_bytes).ok()?))
        .ok_or("atlas-query-busy: memory reservation overflow")
}

pub(crate) fn valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= MAX_REQUEST_ID_BYTES
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn rejected_reply(
    config: QueryServiceConfig,
    request: &QueryServiceRequest,
    now_ms: u64,
    reservation_bytes: u64,
    outcome: QueryOutcome,
    diagnostic: &str,
) -> Box<QueryServiceReply> {
    let retry_after_ms = matches!(outcome, QueryOutcome::Busy | QueryOutcome::Degraded)
        .then_some(config.retry_after_ms);
    Box::new(QueryServiceReply {
        outcome,
        context: None,
        receipt: QueryServiceReceipt {
            schema: RECEIPT_SCHEMA.into(),
            request_id: request.request_id.clone(),
            serving_mode: QueryServingMode::Worker,
            limits: config.into(),
            reservation_bytes,
            enqueued_at_ms: now_ms,
            started_at_ms: None,
            completed_at_ms: now_ms,
            queue_wait_ms: None,
            execution_ms: None,
            attempts: 0,
            outcome,
            generation: request.snapshot.generation().map(str::to_string),
            graph_fingerprint: request.snapshot.graph_fingerprint().to_string(),
            load_profile: None,
            response_digest: None,
            fallback_used: false,
            fallback_generation: None,
        },
        diagnostic: Some(diagnostic.into()),
        retry_after_ms,
        resources: None,
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::*;

    struct FakeClock {
        now_ms: AtomicU64,
    }

    impl FakeClock {
        fn new() -> Self {
            Self {
                now_ms: AtomicU64::new(0),
            }
        }

        fn advance(&self, milliseconds: u64) {
            self.now_ms.fetch_add(milliseconds, Ordering::SeqCst);
        }
    }

    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.now_ms.load(Ordering::SeqCst)
        }
    }

    struct BlockingRunner {
        started: Arc<AtomicUsize>,
        block_count: usize,
        start: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl QueryRunner for BlockingRunner {
        fn run(
            &self,
            _request: &QueryServiceRequest,
            _control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            let position = self.started.fetch_add(1, Ordering::SeqCst);
            if position < self.block_count {
                self.start.wait();
                self.release.wait();
            }
            Err(rust_atlas::AtlasError::Invariant(
                "test runner stopped".into(),
            ))
        }
    }

    struct CancellingRunner {
        started: Arc<Barrier>,
    }

    impl QueryRunner for CancellingRunner {
        fn run(
            &self,
            _request: &QueryServiceRequest,
            control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            self.started.wait();
            loop {
                control.checkpoint()?;
                std::thread::yield_now();
            }
        }
    }

    struct DeadlineRunner {
        calls: Arc<AtomicUsize>,
        started: Arc<Barrier>,
    }

    impl QueryRunner for DeadlineRunner {
        fn run(
            &self,
            _request: &QueryServiceRequest,
            control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.wait();
            loop {
                control.checkpoint()?;
                std::thread::yield_now();
            }
        }
    }

    struct PanicRunner {
        calls: Arc<AtomicUsize>,
    }

    impl QueryRunner for PanicRunner {
        fn run(
            &self,
            _request: &QueryServiceRequest,
            _control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            panic!("injected query runner panic");
        }
    }

    struct SuccessRunner {
        result: rust_atlas::ContextResult,
    }

    impl QueryRunner for SuccessRunner {
        fn run(
            &self,
            _request: &QueryServiceRequest,
            _control: &rust_atlas::ContextExecutionControl,
        ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError> {
            Ok(self.result.clone())
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("agent-spec-query-service-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn fixture(name: &str) -> (PathBuf, PathBuf) {
        let code = temp_dir(name);
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname='query-service-fixture'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn entry() {}\n").unwrap();
        let graph = code.join("graph");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        (code, graph)
    }

    fn request(graph: &Path, request_id: &str) -> QueryServiceRequest {
        QueryServiceRequest {
            request_id: request_id.into(),
            query: "entry".into(),
            options: rust_atlas::ContextOptions {
                frozen: true,
                ..rust_atlas::ContextOptions::default()
            },
            snapshot: rust_atlas::pin_context_snapshot(graph).unwrap(),
        }
    }

    fn next_completion(service: &QueryService) -> QueryServiceReply {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(reply) = service.try_completion() {
                return reply;
            }
            std::thread::yield_now();
        }
        panic!("query service did not complete before the test safety deadline");
    }

    fn reader_lease_count(graph: &Path) -> usize {
        fs::read_dir(graph.join(".runtime/readers"))
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    #[test]
    fn test_atlas_query_service_defaults_direct_and_validates_bounds() {
        let direct = QueryServiceConfig::default();
        assert_eq!(direct.workers, 0);
        assert_eq!(direct.queue_capacity, 4);
        assert_eq!(direct.queue_timeout_ms, 2_000);
        assert_eq!(direct.deadline_ms, 20_000);
        assert_eq!(direct.memory_budget_bytes, 268_435_456);
        assert_eq!(direct.retry_after_ms, 100);
        direct.validate().unwrap();

        let worker = QueryServiceConfig::worker_profile();
        assert_eq!(worker.workers, 2);
        worker.validate().unwrap();

        let disabled = QueryServiceConfig {
            queue_capacity: usize::MAX,
            ..direct
        };
        disabled.validate().unwrap();
        let runner = Arc::new(BlockingRunner {
            started: Arc::new(AtomicUsize::new(0)),
            block_count: 0,
            start: Arc::new(Barrier::new(1)),
            release: Arc::new(Barrier::new(1)),
        });
        let mut disabled_service = QueryService::new(disabled, runner).unwrap();
        assert!(!disabled_service.status().enabled);
        disabled_service.shutdown();

        for (field, invalid) in [
            (
                "workers",
                QueryServiceConfig {
                    workers: 5,
                    ..direct
                },
            ),
            (
                "queue_capacity",
                QueryServiceConfig {
                    workers: 1,
                    queue_capacity: 0,
                    ..direct
                },
            ),
            (
                "queue_timeout_ms",
                QueryServiceConfig {
                    queue_timeout_ms: 9,
                    ..direct
                },
            ),
            (
                "deadline_ms",
                QueryServiceConfig {
                    deadline_ms: 60_001,
                    ..direct
                },
            ),
            (
                "memory_budget_bytes",
                QueryServiceConfig {
                    memory_budget_bytes: 16_777_215,
                    ..direct
                },
            ),
            (
                "retry_after_ms",
                QueryServiceConfig {
                    retry_after_ms: 0,
                    ..direct
                },
            ),
        ] {
            assert_eq!(invalid.validate().unwrap_err().field, field);
        }
    }

    #[test]
    fn test_atlas_query_service_rejects_full_queue_with_typed_busy() {
        let (code, graph) = fixture("full-queue");
        let started = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(3));
        let release = Arc::new(Barrier::new(3));
        let runner = Arc::new(BlockingRunner {
            started: Arc::clone(&started),
            block_count: 2,
            start: Arc::clone(&start),
            release: Arc::clone(&release),
        });
        let mut service = QueryService::new(QueryServiceConfig::worker_profile(), runner).unwrap();

        service.try_submit(request(&graph, "running-1")).unwrap();
        service.try_submit(request(&graph, "running-2")).unwrap();
        start.wait();
        for position in 0..4 {
            service
                .try_submit(request(&graph, &format!("queued-{position}")))
                .unwrap();
        }
        let rejection = service
            .try_submit(request(&graph, "queue-overflow"))
            .unwrap_err();
        assert_eq!(rejection.outcome, QueryOutcome::Busy);
        assert_eq!(rejection.retry_after_ms, Some(100));
        assert!(rejection.context.is_none());
        assert_eq!(started.load(Ordering::SeqCst), 2);

        release.wait();
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_rejects_memory_reservation_over_budget() {
        let (code, graph) = fixture("memory-budget");
        let snapshot = rust_atlas::graph_snapshot(&graph).unwrap();
        let index_path = snapshot.data_dir.join("query-index.json");
        drop(snapshot);
        fs::OpenOptions::new()
            .write(true)
            .open(index_path)
            .unwrap()
            .set_len(3 * 1024 * 1024)
            .unwrap();
        let started = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let runner = Arc::new(BlockingRunner {
            started: Arc::clone(&started),
            block_count: 1,
            start: Arc::clone(&start),
            release: Arc::clone(&release),
        });
        let config = QueryServiceConfig {
            workers: 1,
            memory_budget_bytes: 16_777_216,
            ..QueryServiceConfig::default()
        };
        let mut service = QueryService::new(config, runner).unwrap();

        service.try_submit(request(&graph, "memory-first")).unwrap();
        start.wait();
        let first_reservation = service.status().reserved_bytes;
        assert!(first_reservation > 0);
        assert!(first_reservation < config.memory_budget_bytes);
        let rejection = service
            .try_submit(request(&graph, "memory-aggregate"))
            .unwrap_err();
        assert_eq!(rejection.outcome, QueryOutcome::Busy);
        assert_eq!(service.status().started, 1);
        assert_eq!(service.status().reserved_bytes, first_reservation);
        drop(rejection);

        release.wait();
        let first_reply = next_completion(&service);
        assert_eq!(first_reply.receipt.reservation_bytes, first_reservation);
        assert_eq!(service.status().reserved_bytes, first_reservation);
        assert_eq!(reader_lease_count(&graph), 1);
        drop(first_reply);
        assert_eq!(service.status().reserved_bytes, 0);
        assert_eq!(reader_lease_count(&graph), 0);

        service
            .try_submit(request(&graph, "memory-after-release"))
            .unwrap();
        let after_release = next_completion(&service);
        assert_eq!(after_release.outcome, QueryOutcome::Failed);
        drop(after_release);
        assert_eq!(service.status().reserved_bytes, 0);

        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_expires_queued_job_before_execution() {
        let (code, graph) = fixture("queue-timeout");
        let clock = Arc::new(FakeClock::new());
        let started = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let runner = Arc::new(BlockingRunner {
            started: Arc::clone(&started),
            block_count: 1,
            start: Arc::clone(&start),
            release: Arc::clone(&release),
        });
        let config = QueryServiceConfig {
            workers: 1,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::with_clock(config, runner, clock.clone()).unwrap();

        service.try_submit(request(&graph, "running")).unwrap();
        start.wait();
        service.try_submit(request(&graph, "expiring")).unwrap();
        clock.advance(2_001);
        release.wait();

        let replies = [next_completion(&service), next_completion(&service)];
        let expired = replies
            .iter()
            .find(|reply| reply.receipt.request_id == "expiring")
            .unwrap();
        assert_eq!(expired.outcome, QueryOutcome::Timeout);
        assert_eq!(expired.receipt.attempts, 0);
        assert_eq!(started.load(Ordering::SeqCst), 1);

        drop(replies);
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_cancels_queued_job_before_execution() {
        let (code, graph) = fixture("queued-cancel");
        let started = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let runner = Arc::new(BlockingRunner {
            started: Arc::clone(&started),
            block_count: 1,
            start: Arc::clone(&start),
            release: Arc::clone(&release),
        });
        let config = QueryServiceConfig {
            workers: 1,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::new(config, runner).unwrap();

        service.try_submit(request(&graph, "running")).unwrap();
        start.wait();
        service
            .try_submit(request(&graph, "cancel-queued"))
            .unwrap();
        assert!(service.cancel("cancel-queued"));
        release.wait();

        let replies = [next_completion(&service), next_completion(&service)];
        let cancelled = replies
            .iter()
            .find(|reply| reply.receipt.request_id == "cancel-queued")
            .unwrap();
        assert_eq!(cancelled.outcome, QueryOutcome::Cancelled);
        assert_eq!(cancelled.receipt.attempts, 0);
        assert_eq!(started.load(Ordering::SeqCst), 1);

        drop(replies);
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_times_out_executing_runner_at_checkpoint() {
        let (code, graph) = fixture("execution-timeout");
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(Barrier::new(2));
        let runner = Arc::new(DeadlineRunner {
            calls: Arc::clone(&calls),
            started: Arc::clone(&started),
        });
        let config = QueryServiceConfig {
            workers: 1,
            deadline_ms: 100,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::new(config, runner).unwrap();

        service.try_submit(request(&graph, "deadline")).unwrap();
        started.wait();
        let reply = next_completion(&service);
        assert_eq!(reply.outcome, QueryOutcome::Timeout);
        assert_eq!(reply.receipt.attempts, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(reply.context.is_none());

        drop(reply);
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_cancels_running_projection_and_releases_lease() {
        let (code, graph) = fixture("running-cancel");
        let started = Arc::new(Barrier::new(2));
        let runner = Arc::new(CancellingRunner {
            started: Arc::clone(&started),
        });
        let config = QueryServiceConfig {
            workers: 1,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::new(config, runner).unwrap();

        service.try_submit(request(&graph, "cancel-me")).unwrap();
        started.wait();
        assert!(service.cancel("cancel-me"));
        let reply = next_completion(&service);
        assert_eq!(reply.outcome, QueryOutcome::Cancelled);
        assert!(reply.context.is_none());
        assert_eq!(reader_lease_count(&graph), 1);
        assert_eq!(service.status().outstanding, 1);
        assert!(service.status().reserved_bytes > 0);
        drop(reply);
        assert_eq!(reader_lease_count(&graph), 0);
        assert_eq!(service.status().outstanding, 0);
        assert_eq!(service.status().reserved_bytes, 0);

        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_worker_panic_retries_once_then_opens_circuit() {
        let (code, graph) = fixture("panic-circuit");
        let calls = Arc::new(AtomicUsize::new(0));
        let runner = Arc::new(PanicRunner {
            calls: Arc::clone(&calls),
        });
        let config = QueryServiceConfig {
            workers: 1,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::new(config, runner).unwrap();

        service.try_submit(request(&graph, "panic-twice")).unwrap();
        let reply = next_completion(&service);
        assert_eq!(reply.outcome, QueryOutcome::Degraded);
        assert_eq!(reply.receipt.attempts, 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(service.status().circuit_open);
        assert_eq!(service.status().panics, 2);

        let rejection = service
            .try_submit(request(&graph, "after-circuit"))
            .unwrap_err();
        assert_eq!(rejection.outcome, QueryOutcome::Degraded);
        assert_eq!(rejection.retry_after_ms, Some(100));
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        drop(reply);
        drop(rejection);
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_query_service_outcomes_are_typed_and_mutually_exclusive() {
        let (code, graph) = fixture("typed-outcomes");
        let options = rust_atlas::ContextOptions {
            frozen: true,
            ..rust_atlas::ContextOptions::default()
        };
        let result = rust_atlas::compile_context(&code, &graph, "entry", &options).unwrap();
        let runner = Arc::new(SuccessRunner { result });
        let config = QueryServiceConfig {
            workers: 1,
            ..QueryServiceConfig::worker_profile()
        };
        let mut service = QueryService::new(config, runner).unwrap();
        service
            .try_submit(request(&graph, "success-shape"))
            .unwrap();
        let success = next_completion(&service);
        assert_eq!(success.outcome, QueryOutcome::Success);
        success.validate_shape().unwrap();
        let wire = QueryServiceWireReply::from_reply(&success).unwrap();
        wire.validate_shape().unwrap();
        let mut unknown_field = serde_json::to_value(&wire).unwrap();
        unknown_field
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::Value::Bool(true));
        assert!(
            serde_json::from_value::<QueryServiceWireReply>(unknown_field).is_err(),
            "wire replies reject unknown fields"
        );
        let mut success_shaped_wire_error = wire.clone();
        success_shaped_wire_error.outcome = QueryOutcome::Busy;
        success_shaped_wire_error.receipt.outcome = QueryOutcome::Busy;
        success_shaped_wire_error.diagnostic = Some("busy".to_string());
        success_shaped_wire_error.retry_after_ms = Some(100);
        assert!(success_shaped_wire_error.validate_shape().is_err());
        assert!(!service.cancel("success-shape"));

        for outcome in [
            QueryOutcome::Busy,
            QueryOutcome::Timeout,
            QueryOutcome::Cancelled,
            QueryOutcome::Degraded,
            QueryOutcome::Failed,
            QueryOutcome::Unavailable,
        ] {
            let mut reply = success.clone();
            reply.outcome = outcome;
            reply.receipt.outcome = outcome;
            reply.context = None;
            reply.diagnostic = Some(format!("typed {outcome:?}"));
            reply.retry_after_ms =
                matches!(outcome, QueryOutcome::Busy | QueryOutcome::Degraded).then_some(100);
            reply.validate_shape().unwrap();
        }

        let mut success_shaped_busy = success.clone();
        success_shaped_busy.outcome = QueryOutcome::Busy;
        success_shaped_busy.receipt.outcome = QueryOutcome::Busy;
        success_shaped_busy.diagnostic = Some("busy".into());
        success_shaped_busy.retry_after_ms = Some(100);
        assert!(success_shaped_busy.validate_shape().is_err());

        let mut mismatched_receipt = success.clone();
        mismatched_receipt.receipt.outcome = QueryOutcome::Failed;
        assert!(mismatched_receipt.validate_shape().is_err());

        let mut retryless_busy = success;
        retryless_busy.outcome = QueryOutcome::Busy;
        retryless_busy.receipt.outcome = QueryOutcome::Busy;
        retryless_busy.context = None;
        retryless_busy.diagnostic = Some("busy".into());
        retryless_busy.retry_after_ms = None;
        assert!(retryless_busy.validate_shape().is_err());

        drop(success_shaped_busy);
        drop(mismatched_receipt);
        drop(retryless_busy);
        service.shutdown();
        drop(service);
        fs::remove_dir_all(code).ok();
    }
}
