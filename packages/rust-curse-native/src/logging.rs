use std::io::IsTerminal;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Default filter when `RUST_CURSE_LOG` is unset
const DEFAULT_DIRECTIVES: &str = "rust_curse=info";
const ELAPSED_WIDTH: usize = 6;

/// Install the stderr subscriber the first time it is called.
pub fn ensure() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        let directives =
            std::env::var("RUST_CURSE_LOG").unwrap_or_else(|_| DEFAULT_DIRECTIVES.to_string());
        let env_filter =
            EnvFilter::try_new(&directives).unwrap_or_else(|_| EnvFilter::new(DEFAULT_DIRECTIVES));

        let _ = tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_ansi(use_color())
                    .with_timer(ElapsedTimer::new())
                    .with_writer(std::io::stderr),
            )
            .try_init();
    });
}

fn use_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    std::io::stderr().is_terminal()
}

#[derive(Clone, Debug)]
struct ElapsedTimer {
    start: Instant,
}

impl ElapsedTimer {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl FormatTime for ElapsedTimer {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", format_elapsed_aligned(self.start.elapsed()))
    }
}

fn format_elapsed_aligned(elapsed: Duration) -> String {
    format!("{:>ELAPSED_WIDTH$}", format_elapsed(elapsed))
}

fn format_elapsed(elapsed: Duration) -> String {
    format!("{}ms", elapsed.as_millis())
}
