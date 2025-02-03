use log::{info, warn, error};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn init() {
    // Initialize the logger with the default log level (e.g., from RUST_LOG environment variable)
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let log_path = env::var("LOG_PATH").unwrap_or_else(|_| "/tmp/logs".to_string());

    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .parse_filters(&log_level)
        .init();

    // Log an initialization message
    info!("Logging initialized with level: {}", log_level);
}

pub fn log_event(module: &str, event: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    info!("[{}] {}: {}", timestamp, module, event);
}

pub fn log_warning(module: &str, warning: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    warn!("[{}] {}: {}", timestamp, module, warning);
}

pub fn log_error(module: &str, error: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    error!("[{}] {}: {}", timestamp, module, error);
}
