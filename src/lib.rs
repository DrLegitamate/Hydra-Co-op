//! Hydra Co-op Launcher Library
//! 
//! This library provides the core functionality for the Hydra Co-op Launcher,
//! a tool designed for Linux to simplify setting up local split-screen 
//! co-operative gameplay by launching and managing multiple instances of a game.

pub mod cli;
pub mod config;
pub mod errors;
pub mod gui;
pub mod input_mux;
pub mod instance_manager;
pub mod logging;
pub mod net_emulator;
pub mod proton_integration;
pub mod window_manager;

// Re-export commonly used types
pub use errors::{HydraError, Result};
pub use config::Config;
pub use window_manager::Layout;
pub use input_mux::{DeviceIdentifier, InputAssignment, InputMux};

/// Application metadata
pub const APP_NAME: &str = "Hydra Co-op Launcher";
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
    use std::path::{Path, PathBuf};
    use crate::Result;
    
    /// Get the default configuration directory
    pub fn get_config_dir() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|dir| dir.join("hydra-coop"))
            .ok_or_else(|| crate::HydraError::application("Could not determine config directory"))
    }
    
    /// Get the default data directory
    pub fn get_data_dir() -> Result<PathBuf> {
        dirs::data_dir()
            .map(|dir| dir.join("hydra-coop"))
            .ok_or_else(|| crate::HydraError::application("Could not determine data directory"))
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
            return Err(crate::HydraError::validation(format!(
                "Executable not found: {}", path.display()
            )));
        }
        
        if !path.is_file() {
            return Err(crate::HydraError::validation(format!(
                "Path is not a file: {}", path.display()
            )));
        }
        
        // On Unix systems, check if the file is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(crate::HydraError::validation(format!(
                    "File is not executable: {}", path.display()
                )));
            }
        }
        
        Ok(())
    }
}