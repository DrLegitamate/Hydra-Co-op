use log::{LevelFilter, SetLoggerError};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

/// Initialise the logging system.
///
/// Log level is read from the `RUST_LOG` environment variable (default: `info`).
/// If `LOG_PATH` is set, log output is written to **both** stdout and that file
/// (append mode, created automatically with parent directories).
pub fn init() -> Result<(), SetLoggerError> {
    let log_level_str = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let level = parse_level(&log_level_str);

    let fmt = |out: fern::FormatCallback, message: &std::fmt::Arguments, record: &log::Record| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));

        out.finish(format_args!(
            "[{:05}.{:06} {} {}] {}",
            now.as_secs(),
            now.subsec_micros(),
            record.level(),
            record.module_path().unwrap_or(""),
            message
        ))
    };

    let mut dispatch = fern::Dispatch::new()
        .format(fmt)
        .level(level)
        .chain(std::io::stdout());

    if let Ok(path_str) = env::var("LOG_PATH") {
        // Ensure the parent directory exists before opening the file.
        let log_path = std::path::Path::new(&path_str);
        if let Some(parent) = log_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("Could not create log directory '{}': {}", parent.display(), e);
            }
        }
        match fern::log_file(&path_str) {
            Ok(file) => {
                dispatch = dispatch.chain(file);
                eprintln!("Logging to file: {}", path_str);
            }
            Err(e) => {
                eprintln!("Could not open log file '{}': {} — logging to stdout only.", path_str, e);
            }
        }
    }

    dispatch.apply()
}

/// Convenience initialiser with an explicit level (ignores `RUST_LOG`).
/// Also respects `LOG_PATH` for dual output.
pub fn init_with_level(level: LevelFilter) -> Result<(), SetLoggerError> {
    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0));

            let colour = match record.level() {
                log::Level::Error => "\x1b[31m",
                log::Level::Warn  => "\x1b[33m",
                log::Level::Info  => "\x1b[32m",
                log::Level::Debug => "\x1b[36m",
                log::Level::Trace => "\x1b[35m",
            };

            out.finish(format_args!(
                "[{:05}.{:06}] {}{:5}\x1b[0m [{}] {}",
                now.as_secs(),
                now.subsec_micros(),
                colour,
                record.level(),
                record.module_path().unwrap_or("unknown"),
                message
            ))
        })
        .level(level)
        .chain(std::io::stdout());

    if let Ok(path_str) = env::var("LOG_PATH") {
        let log_path = std::path::Path::new(&path_str);
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(file) = fern::log_file(&path_str) {
            dispatch = dispatch.chain(file);
        }
    }

    dispatch.apply()
}

fn parse_level(s: &str) -> LevelFilter {
    match s.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn"  => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _       => LevelFilter::Info,
    }
}
