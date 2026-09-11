use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, Mutex},
    time::Instant,
};

use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
use tracing_subscriber::{Layer, Registry, layer::Context, prelude::*, registry::LookupSpan};

use crate::server::{ServerLog, ServerLogLevel};

const BUFFER_CAPACITY: usize = 1024;

type LogCallback = ThreadsafeFunction<ServerLog, (), ServerLog, napi::Status, false, true>;

/// A server-local subscriber. Callers explicitly scope work with `dispatch()`;
/// nothing is installed globally or implicitly propagated to worker threads.
pub struct ServerLogs {
    collector: Arc<Mutex<Collector>>,
    dispatch: Dispatch,
}

impl ServerLogs {
    pub fn new() -> Self {
        let collector = Arc::new(Mutex::new(Collector::default()));
        let dispatch = Dispatch::new(Registry::default().with(LogLayer(collector.clone())));
        Self {
            collector,
            dispatch,
        }
    }

    pub fn dispatch(&self) -> &Dispatch {
        &self.dispatch
    }

    /// Replace the callback and flush buffered logs in order. Detaching resumes
    /// buffering. Calls already queued for the previous callback cannot be
    /// revoked.
    pub fn on_log(&self, callback: Option<LogCallback>) {
        let mut collector = self.collector.lock().unwrap_or_else(|err| err.into_inner());
        collector.callback = callback;
        if collector.callback.is_some() {
            while let Some(log) = collector.pending.pop_front() {
                collector.emit(log);
            }
        }
    }
}

#[derive(Default)]
struct Collector {
    callback: Option<LogCallback>,
    pending: VecDeque<ServerLog>,
}

impl Collector {
    fn emit(&mut self, log: ServerLog) {
        if let Some(callback) = &self.callback {
            // Enqueue while holding the collector lock to preserve flush/event
            // ordering. This never waits for JS and does not keep Node alive.
            // Delivery is best-effort during shutdown; do not recursively log
            // callback failures through this subscriber.
            let _ = callback.call(log, ThreadsafeFunctionCallMode::NonBlocking);
        } else {
            // Keep the newest startup/detached logs, dropping the oldest first.
            if self.pending.len() == BUFFER_CAPACITY {
                self.pending.pop_front();
            }
            self.pending.push_back(log);
        }
    }
}

struct LogLayer(Arc<Mutex<Collector>>);

struct SpanState {
    fields: LogFields,
    started: Instant,
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        // Filter before recording: RA/Salsa debug fields can attempt to format
        // solver internals that are unsafe to inspect outside their context.
        let target = metadata.target();
        target == "rust_curse" || target.starts_with("rust_curse::")
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut fields = LogFields::default();
            attrs.record(&mut fields);
            span.extensions_mut().insert(SpanState {
                fields,
                started: Instant::now(),
            });
            let mut fields = HashMap::new();
            for ancestor in span.scope().from_root() {
                if let Some(state) = ancestor.extensions().get::<SpanState>() {
                    fields.extend(state.fields.0.clone());
                }
            }
            self.emit(span.metadata(), format!("{} starting", span.name()), fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(state) = extensions.get_mut::<SpanState>() {
                values.record(&mut state.fields);
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = HashMap::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(state) = span.extensions().get::<SpanState>() {
                    fields.extend(state.fields.0.clone());
                }
            }
        }

        let mut event_fields = LogFields::default();
        event.record(&mut event_fields);
        let message = event_fields.0.remove("message").unwrap_or_default();
        fields.extend(event_fields.0);

        self.emit(event.metadata(), message, fields);
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let Some(started) = span
            .extensions()
            .get::<SpanState>()
            .map(|state| state.started)
        else {
            return;
        };
        let mut fields = HashMap::new();
        for ancestor in span.scope().from_root() {
            if let Some(state) = ancestor.extensions().get::<SpanState>() {
                fields.extend(state.fields.0.clone());
            }
        }
        // Wall-clock lifetime, including nested spans and idle time. Parent
        // durations include child durations and should not be summed with them.
        fields.insert(
            "elapsed_ms".to_owned(),
            started.elapsed().as_millis().to_string(),
        );
        self.emit(span.metadata(), format!("{} finished", span.name()), fields);
    }
}

impl LogLayer {
    fn emit(&self, metadata: &Metadata<'_>, message: String, fields: HashMap<String, String>) {
        let level = match *metadata.level() {
            tracing::Level::ERROR => ServerLogLevel::Error,
            tracing::Level::WARN => ServerLogLevel::Warn,
            tracing::Level::INFO => ServerLogLevel::Info,
            tracing::Level::DEBUG => ServerLogLevel::Debug,
            tracing::Level::TRACE => ServerLogLevel::Trace,
        };
        self.0
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .emit(ServerLog {
                level,
                target: metadata.target().to_owned(),
                message,
                fields,
            });
    }
}

#[derive(Default)]
struct LogFields(HashMap<String, String>);

impl Visit for LogFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}
