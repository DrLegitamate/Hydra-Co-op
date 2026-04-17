//! Hydra Co-op Launcher Library
//!
//! This library provides the core functionality for the Hydra Co-op Launcher,
//! a tool designed for Linux to simplify setting up local split-screen
//! co-operative gameplay by launching and managing multiple instances of a game.

pub mod adaptive_config;
pub mod cli;
pub mod config;
pub mod errors;
pub mod game_detection;
pub mod input_mux;
pub mod logging;
pub mod net_emulator;
pub mod proton_integration;
pub mod universal_launcher;
pub mod window_manager;

// The `gui` module is binary-only (src/main.rs declares it) because it
// depends on binary-only helpers such as `run_core_logic`.

// Re-export commonly used types
pub use adaptive_config::AdaptiveConfigManager;
pub use config::Config;
pub use errors::{HydraError, Result};
pub use game_detection::{GameConfiguration, GameDetector, GameProfile};
pub use input_mux::{DeviceIdentifier, InputAssignment, InputMux};
pub use universal_launcher::{GameInstance, UniversalLauncher};
pub use window_manager::Layout;

/// Application metadata
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Default configuration values
pub mod defaults {
    use std::time::Duration;

    pub const MAX_INSTANCES: usize = 8;
    pub const DEFAULT_INSTANCES: usize = 2;
    pub const WINDOW_SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
    pub const NETWORK_TIMEOUT: Duration = Duration::from_millis(100);
    pub const INPUT_POLL_TIMEOUT: Duration = Duration::from_millis(100);
}

/// Utility functions
pub mod utils {
    use crate::{HydraError, Result};
    use std::path::{Path, PathBuf};

    /// Get the default configuration directory
    pub fn get_config_dir() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|dir| dir.join("hydra-coop"))
            .ok_or_else(|| HydraError::application("Could not determine config directory"))
    }

    /// Get the default data directory
    pub fn get_data_dir() -> Result<PathBuf> {
        dirs::data_dir()
            .map(|dir| dir.join("hydra-coop"))
            .ok_or_else(|| HydraError::application("Could not determine data directory"))
    }

    /// Ensure a directory exists, creating it if necessary
    pub fn ensure_dir_exists(path: &Path) -> Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// Validate that a file exists and is executable
    pub fn validate_executable(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(HydraError::validation(format!(
                "Executable not found: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(HydraError::validation(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path)?;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(HydraError::validation(format!(
                    "File is not executable: {}",
                    path.display()
                )));
            }
        }

        Ok(())
    }
}
