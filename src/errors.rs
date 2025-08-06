//! Centralized error handling for the Hydra Co-op Launcher
//! 
//! This module provides a unified error type that wraps all the various
//! error types used throughout the application, making error handling
//! more consistent and easier to manage.

use thiserror::Error;
use std::io;

/// Main error type for the Hydra Co-op Launcher application
#[derive(Error, Debug)]
pub enum HydraError {
    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),
    
    #[error("Input multiplexer error: {0}")]
    InputMux(#[from] crate::input_mux::InputMuxError),
    
    #[error("Instance manager error: {0}")]
    InstanceManager(#[from] crate::instance_manager::InstanceManagerError),
    
    #[error("Network emulator error: {0}")]
    NetEmulator(#[from] crate::net_emulator::NetEmulatorError),
    
    #[error("Window manager error: {0}")]
    WindowManager(#[from] crate::window_manager::WindowManagerError),
    
    #[error("Proton integration error: {0}")]
    Proton(#[from] crate::proton_integration::ProtonError),
    
    #[error("Game detection error: {0}")]
    GameDetection(#[from] crate::game_detection::GameDetectionError),
    
    #[error("Universal launcher error: {0}")]
    UniversalLauncher(#[from] crate::universal_launcher::UniversalLauncherError),
    
    #[error("Adaptive config error: {0}")]
    AdaptiveConfig(#[from] crate::adaptive_config::AdaptiveConfigError),
    
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Logging initialization error: {0}")]
    Logging(#[from] log::SetLoggerError),
    
    #[error("Application error: {0}")]
    Application(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, HydraError>;

impl HydraError {
    /// Create a new application error
    pub fn application(msg: impl Into<String>) -> Self {
        HydraError::Application(msg.into())
    }
    
    /// Create a new validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        HydraError::Validation(msg.into())
    }
}