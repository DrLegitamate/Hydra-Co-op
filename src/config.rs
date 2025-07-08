use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::fs;
use std::path::{Path, PathBuf};
use log::{info, warn, error, debug}; // Import log macros
use std::error::Error; // Import Error trait
use toml; // Explicitly import toml

/// Configuration validation errors
#[derive(Debug)]
pub enum ValidationError {
    InvalidInstanceCount(usize),
    InvalidNetworkPort(u16),
    MissingGamePath,
    InvalidGamePath(PathBuf),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ValidationError::InvalidInstanceCount(count) => {
                write!(f, "Invalid instance count: {}. Must be between 1 and {}", count, crate::defaults::MAX_INSTANCES)
            }
            ValidationError::InvalidNetworkPort(port) => {
                write!(f, "Invalid network port: {}. Must be between 1024 and 65535", port)
            }
            ValidationError::MissingGamePath => {
                write!(f, "No game executable path specified")
            }
            ValidationError::InvalidGamePath(path) => {
                write!(f, "Invalid game executable path: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ValidationError {}

// Custom error type for configuration operations
#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    TomlDeError(toml::de::Error),
    TomlSeError(toml::ser::Error),
    GenericError(String),
    Validation(ValidationError),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "Configuration I/O error: {}", e),
            ConfigError::TomlDeError(e) => write!(f, "Configuration deserialization error: {}", e),
            ConfigError::TomlSeError(e) => write!(f, "Configuration serialization error: {}", e),
            ConfigError::GenericError(msg) => write!(f, "Configuration error: {}", msg),
            ConfigError::Validation(e) => write!(f, "Configuration validation error: {}", e),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::IoError(e) => Some(e),
            ConfigError::TomlDeError(e) => Some(e),
            ConfigError::TomlSeError(e) => Some(e),
            ConfigError::Validation(e) => Some(e),
            _ => None,
        }
    }
}

// Implement From conversions for easier error propagation
impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::IoError(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::TomlDeError(err)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(err: toml::ser::Error) -> Self {
        ConfigError::TomlSeError(err)
    }
}

impl From<ValidationError> for ConfigError {
    fn from(err: ValidationError) -> Self {
        ConfigError::Validation(err)
    }
}

/// Represents the application's configuration.
#[derive(Debug, Serialize, Deserialize, Clone, Default)] // Added Default derive
pub struct Config {
    pub game_paths: Vec<PathBuf>, // Use PathBuf for paths
    pub input_mappings: Vec<String>, // Store input mappings (names or serialized IDs)
    pub window_layout: String, // Store layout as a string (e.g., "horizontal", "vertical")
    pub network_ports: Vec<u16>, // Ports the game instances use for network communication
    pub use_proton: bool, // Added use_proton field
    // Add other configuration fields as needed (e.g., Proton path, advanced settings)
}

impl Config {
    /// Loads the configuration from a TOML file.
    /// If the file does not exist, returns the default configuration.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        info!("Attempting to load configuration from {}", path.display());
        match fs::read_to_string(path) {
            Ok(contents) => {
                debug!("Read config file contents:\n{}", contents);
                // Use the ? operator after mapping the error
                let config: Config = toml::from_str(&contents)?;
                Ok(config)
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                warn!("Configuration file not found at {}. Using default configuration.", path.display());
                Ok(Config::default_config())
            }
            Err(e) => {
                // Map other IO errors and use ?
                error!("Failed to read configuration file {}: {}", path.display(), e);
                Err(ConfigError::IoError(e))
            }
        }
    }

    /// Saves the current configuration to a TOML file.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        info!("Attempting to save configuration to {}", path.display());

        // Ensure the parent directory exists before saving
        if let Some(parent) = path.parent() {
            // Use ? operator for directory creation and map the error
            fs::create_dir_all(parent).map_err(ConfigError::IoError)?;
        } else {
            warn!("Config path {} has no parent directory. Saving to root?", path.display());
        }

        // Use ? operator after mapping the serialization error
        let toml_string = toml::to_string_pretty(self)?;
        debug!("Saving config contents:\n{}", toml_string);

        // Use ? operator for file creation and map the error
        let mut file = fs::File::create(path)?;
        // Use ? operator for writing and map the error
        file.write_all(toml_string.as_bytes())?;

        info!("Configuration saved successfully to {}", path.display());
        Ok(())
    }

    /// Returns the default application configuration.
    pub fn default_config() -> Config {
        info!("Generating default configuration.");
        Config {
            game_paths: Vec::new(),
            input_mappings: vec!["Auto-detect".to_string(), "Auto-detect".to_string()], // Default for 2 players
            window_layout: "horizontal".to_string(), // Default layout
            network_ports: vec![7777, 7778], // Example default ports for 2 instances
            use_proton: false, // Default to not using Proton
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate game paths
        if self.game_paths.is_empty() {
            return Err(ValidationError::MissingGamePath.into());
        }
        
        for path in &self.game_paths {
            if !path.exists() {
                return Err(ValidationError::InvalidGamePath(path.clone()).into());
            }
        }
        
        // Validate instance count based on input mappings
        let instance_count = self.input_mappings.len();
        if instance_count == 0 || instance_count > crate::defaults::MAX_INSTANCES {
            return Err(ValidationError::InvalidInstanceCount(instance_count).into());
        }
        
        // Validate network ports
        for &port in &self.network_ports {
            if port < 1024 || port == 0 {
                return Err(ValidationError::InvalidNetworkPort(port).into());
            }
        }
        
        Ok(())
    }
    
    /// Get the primary game executable path
    pub fn primary_game_path(&self) -> Option<&PathBuf> {
        self.game_paths.first()
    }
    
    /// Get the number of instances based on input mappings
    pub fn instance_count(&self) -> usize {
        self.input_mappings.len().max(1)
    }
    
    /// Merge this configuration with another, with the other taking precedence
    pub fn merge_with(&mut self, other: Config) {
        if !other.game_paths.is_empty() {
            self.game_paths = other.game_paths;
        }
        if !other.input_mappings.is_empty() {
            self.input_mappings = other.input_mappings;
        }
        if other.window_layout != "horizontal" {
            self.window_layout = other.window_layout;
        }
        if !other.network_ports.is_empty() {
            self.network_ports = other.network_ports;
        }
        // use_proton is always merged
        self.use_proton = other.use_proton;
    }
}

// Test code (add necessary dependencies like tempfile)
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir; // Add tempfile = "3.2" to Cargo.toml
    use std::fs;

    // Helper to set up a basic logger for tests if needed
    // use env_logger;
    // fn setup_logger() {
    //     let _ = env_logger::builder().is_test(true).try_init();
    // }

    #[test]
    fn test_default_config() {
        // setup_logger();
        let config = Config::default_config();
        assert_eq!(config.game_paths.len(), 0);
        assert_eq!(config.input_mappings, vec!["Auto-detect".to_string(), "Auto-detect".to_string()]);
        assert_eq!(config.window_layout, "horizontal".to_string());
        assert_eq!(config.network_ports, vec![7777, 7778]);
        assert_eq!(config.use_proton, false);
    }

    #[test]
    fn test_save_and_load_config() {
        // setup_logger();
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let config_path = temp_dir.path().join("test_config.toml");

        let mut config = Config::default_config();
        config.game_paths.push(PathBuf::from("/path/to/game"));
        config.input_mappings = vec!["Device A".to_string(), "Device B".to_string()];
        config.window_layout = "vertical".to_string();
        config.network_ports = vec![1234, 5678];
        config.use_proton = true;

        // Save the configuration
        let save_result = config.save(&config_path);
        assert!(save_result.is_ok(), "Failed to save config: {:?}", save_result.err());

        // Check if the file was actually created
        assert!(config_path.exists());

        // Load the configuration from the saved file
        let loaded_config_result = Config::load(&config_path);
        assert!(loaded_config_result.is_ok(), "Failed to load config: {:?}", loaded_config_result.err());

        let loaded_config = loaded_config_result.unwrap();

        // Assert that the loaded configuration matches the saved configuration
        assert_eq!(loaded_config.game_paths, vec![PathBuf::from("/path/to/game")]);
        assert_eq!(loaded_config.input_mappings, vec!["Device A".to_string(), "Device B".to_string()]);
        assert_eq!(loaded_config.window_layout, "vertical".to_string());
        assert_eq!(loaded_config.network_ports, vec![1234, 5678]);
        assert_eq!(loaded_config.use_proton, true);

        // Clean up the temporary directory
        // temp_dir is automatically cleaned up when it goes out of scope
    }

    #[test]
    fn test_load_nonexistent_config() {
        // setup_logger();
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let config_path = temp_dir.path().join("nonexistent_config.toml");

        // Attempt to load a non-existent file
        let loaded_config_result = Config::load(&config_path);
        assert!(loaded_config_result.is_ok(), "Loading non-existent config should return Ok");

        // Assert that the default configuration is returned
        let loaded_config = loaded_config_result.unwrap();
        let default_config = Config::default_config();
        assert_eq!(loaded_config.game_paths, default_config.game_paths);
        assert_eq!(loaded_config.input_mappings, default_config.input_mappings);
        assert_eq!(loaded_config.window_layout, default_config.window_layout);
        assert_eq!(loaded_config.network_ports, default_config.network_ports);
        assert_eq!(loaded_config.use_proton, default_config.use_proton);
    }

    #[test]
    fn test_save_to_nonexistent_directory() {
         // setup_logger();
         let temp_dir = tempdir().expect("Failed to create temporary directory");
         let non_existent_subdir = temp_dir.path().join("non_existent_subdir");
         let config_path = non_existent_subdir.join("test_config_in_subdir.toml");

         let config = Config::default_config();

         // Save the configuration to a path in a non-existent directory
         let save_result = config.save(&config_path);
         assert!(save_result.is_ok(), "Failed to save config to non-existent directory: {:?}", save_result.err());

         // Check if the directory and file were created
         assert!(non_existent_subdir.exists());
         assert!(config_path.exists());

         // Clean up the temporary directory
         // temp_dir is automatically cleaned up when it goes out of scope
    }

     #[test]
     fn test_load_invalid_toml() {
         // setup_logger();
         let temp_dir = tempdir().expect("Failed to create temporary directory");
         let config_path = temp_dir.path().join("invalid_config.toml");

         // Write invalid TOML content to the file
         let invalid_toml = r#"
         game_paths = [ "/path/to/game"
         input_mappings = ["Device A", "Device B"]
         "#; // Missing closing bracket for game_paths

         fs::write(&config_path, invalid_toml).expect("Failed to write invalid TOML");

         // Attempt to load the invalid configuration
         let loaded_config_result = Config::load(&config_path);

         // Assert that the loading failed with a TomlDeError
         assert!(loaded_config_result.is_err());
         match loaded_config_result.unwrap_err() {
             ConfigError::TomlDeError(_) => { /* Correct error type */ },
             other => panic!("Expected TomlDeError, but got {:?}", other),
         }

         // Clean up the temporary directory
         // temp_dir is automatically cleaned up when it goes out of scope
     }
}
