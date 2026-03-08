use pact_graphql_plugin::server;
use std::path::PathBuf;

use tracing::info;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    server::run().await
}

fn init_tracing() {
    let filter = log_filter();
    if let Some((appender, log_path)) = log_file_appender() {
        write_env_marker(&log_path);
        write_startup_version(&log_path);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        // Leak guard so the background worker lives for the process lifetime.
        Box::leak(Box::new(guard));
        let init_result = fmt()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .with_target(false)
            .try_init();
        write_tracing_init_result(&log_path, init_result.as_ref().err().map(|err| &**err));
        info!(log_path = %log_path.display(), "logging to file");
    } else {
        let _ = fmt().with_env_filter(filter).with_target(false).try_init();
    }
}

fn log_filter() -> EnvFilter {
    let rust_log = std::env::var("RUST_LOG").ok();
    if let Some(value) = rust_log.as_deref() {
        if !value.trim().is_empty() && !value.eq_ignore_ascii_case("off") {
            return EnvFilter::new(value);
        }
    }

    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string());
    if log_level.trim().is_empty() || log_level.eq_ignore_ascii_case("off") {
        EnvFilter::new("off")
    } else {
        EnvFilter::new(log_level)
    }
}

fn log_file_appender() -> Option<(rolling::RollingFileAppender, PathBuf)> {
    let log_dir = if let Ok(dir) = std::env::var("PACT_PLUGIN_DIR") {
        PathBuf::from(dir)
    } else if let Ok(exe) = std::env::current_exe() {
        exe.parent().map(PathBuf::from)?
    } else if let Ok(home) = std::env::var("HOME") {
        let mut dir = PathBuf::from(home);
        dir.push(".pact");
        dir.push("plugins");
        dir.push("graphql-0.1.0");
        dir
    } else {
        return None;
    };

    std::fs::create_dir_all(&log_dir).ok()?;
    let log_path = log_dir.join("graphql-plugin.log");
    Some((rolling::never(log_dir, "graphql-plugin.log"), log_path))
}

fn write_startup_version(path: &PathBuf) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "plugin version={}", env!("CARGO_PKG_VERSION"));
    }
}

fn write_env_marker(path: &PathBuf) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "<unset>".to_string());
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "<unset>".to_string());
        let plugin_dir = std::env::var("PACT_PLUGIN_DIR").unwrap_or_else(|_| "<unset>".to_string());
        let _ = writeln!(file, "env RUST_LOG={}", rust_log);
        let _ = writeln!(file, "env LOG_LEVEL={}", log_level);
        let _ = writeln!(file, "env PACT_PLUGIN_DIR={}", plugin_dir);
    }
}

fn write_tracing_init_result(path: &PathBuf, err: Option<&(dyn std::error::Error + Send + Sync)>) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        match err {
            Some(err) => {
                let _ = writeln!(file, "tracing init failed: {}", err);
            }
            None => {
                let _ = writeln!(file, "tracing init ok");
            }
        }
    }
}
