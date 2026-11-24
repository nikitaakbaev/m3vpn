use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct LoggingGuard {
    pub _guard: WorkerGuard,
    pub dir: PathBuf,
}

pub fn init(dir: &Path) -> Result<LoggingGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::daily(dir, "m3vpn.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let result = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .try_init();

    if let Err(error) = result {
        eprintln!("tracing already initialized: {error}");
    }

    Ok(LoggingGuard {
        _guard: guard,
        dir: dir.to_path_buf(),
    })
}
