use log::{LevelFilter, SetLoggerError};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

fn parse_level(s: &str) -> LevelFilter {
    match s.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn"  => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _       => LevelFilter::Info,
    }
}
