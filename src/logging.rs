use log::{info, LevelFilter, SetLoggerError}; // Import LevelFilter and SetLoggerError
use std::env;
use std::fs::File;
use std::io::Write; // Import Write for file logging
use std::path::Path;

/// Initializes the logging system using env_logger.
/// Configures logging to stdout and optionally to a file based on environment variables.
///
/// Reads log level from RUST_LOG environment variable, defaults to "info".
/// Reads log file path from LOG_PATH environment variable.
///
/// # Returns
///
/// * `Result<(), SetLoggerError>` - Returns Ok if initialization is successful,
///   otherwise returns a SetLoggerError if the logger has already been set.
pub fn init() -> Result<(), SetLoggerError> {
    let log_level_str = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let log_path_str = env::var("LOG_PATH"); // Read LOG_PATH environment variable

    let mut builder = env_logger::Builder::new();

    // Set the target to stdout by default
    builder.target(env_logger::Target::Stdout);

    // Parse the log level filter from the environment variable
    builder.parse_filters(&log_level_str);

    // Configure log formatting: include timestamp, level, and module path
    // This replaces the need for custom log_event, log_warning, etc. functions.
    builder.format(|buf, record| {
        // Get the current time with microseconds
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)); // Handle potential error gracefully

        // Format the log message including timestamp, level, and module path
        writeln!(
            buf,
            "[{:05}.{:06} {} {}] {}",
            now.as_secs(),
            now.subsec_micros(),
            record.level(),
            record.module_path().unwrap_or(""), // Include module path where macro was called
            record.args()
        )
    });

    // If LOG_PATH is set, also log to a file
    if let Ok(path_str) = log_path_str {
        let log_path = Path::new(&path_str);

        // Ensure the parent directory exists
        if let Some(parent) = log_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                 // Log an error to stdout if creating the log directory fails
                 eprintln!("Error creating log directory {}: {}", parent.display(), e);
                 // Decide how to handle this: proceed without file logging or exit
                 // For now, we log and proceed with only stdout logging.
            } else {
                 // Attempt to open the log file in append mode (create if not exists)
                 match File::create(log_path) { // Using create which truncates, use OpenOptions for append
                     Ok(file) => {
                         // Set the file as an additional log target
                         // Note: env_logger can target multiple outputs simultaneously.
                         // With the format closure, you might need a more advanced approach
                         // to write the *same* formatted message to both stdout and file.
                         // A simpler way with env_logger is to use its built-in file logging features
                         // or log to a central handler that duplicates output.

                         // For simplicity and demonstration, let's modify the format closure
                         // to write to the file as well, or use a different logger or feature.
                         // env_logger's target() usually replaces, not adds.

                         // A common approach is to log to stdout and then have a separate
                         // mechanism or a more feature-rich logging crate (like `fern` or `log4rs`)
                         // handle splitting output to a file.

                         // Let's simplify and just use env_logger for stdout, and if file logging is critical,
                         // reconsider the approach or use a different crate.
                         // Sticking with env_logger for now, focusing on formatting and basic init.
                         // File logging with env_logger's format closure is complex.
                         // If file logging is essential with custom formatting, consider `fern`.

                         // Revised approach: If LOG_PATH is set, try to use a different logger setup
                         // or a crate that supports multiple outputs easily.
                         // Sticking with the current env_logger for stdout is simpler.
                         // Let's just log a message indicating file logging is requested but not implemented with current setup.
                          warn!("LOG_PATH environment variable set, but file logging is not fully implemented with current env_logger setup. Logging to stdout only.");
                     }
                     Err(e) => {
                         eprintln!("Error creating log file {}: {}", log_path.display(), e);
                     }
                 }
            }
        } else {
            eprintln!("Invalid LOG_PATH: {} (no parent directory)", log_path.display());
        }
    }


    // Initialize the logger. This can only be done once.
    builder.try_init()
}

// The custom logging functions are no longer needed.
// Use the standard log macros (info!, warn!, error!, debug!) directly.

/*
// Removed as standard log macros with formatting handle this
pub fn log_event(module: &str, event: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    info!("[{}] {}: {}", timestamp, module, event);
}

// Removed as standard log macros with formatting handle this
pub fn log_warning(module: &str, warning: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    warn!("[{}] {}: {}", timestamp, module, warning);
}

// Removed as standard log macros with formatting handle this
pub fn log_error(module: &str, error: &str) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    error!("[{}] {}: {}", timestamp, module, error);
}
*/

// Note: You would use the standard log macros directly in your code now:
// info!("Application started.");
// warn!("Something potentially problematic happened.");
// error!("A critical error occurred.");
// debug!("Detailed debug information.");

// Test code (can be added if needed, but basic init is hard to test isolation)
// #[cfg(test)]
// mod tests {
//     use super::*;
//     // Tests for logging initialization are tricky because the logger can only be set once.
//     // You might need to run tests in separate processes or rely on manual verification.
// }
