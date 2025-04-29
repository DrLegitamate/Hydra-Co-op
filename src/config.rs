use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::fs;
use std::path::{Path, PathBuf};
use log::{info, warn, error, debug}; // Import log macros
use std::error::Error; // Import Error trait

// Custom error type for configuration operations
#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    TomlDeError(toml::de::Error),
    TomlSeError(toml::ser::Error),
    GenericError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "Configuration I/O error: {}", e),
            ConfigError::TomlDeError(e) => write!(f, "Configuration deserialization error: {}", e),
            ConfigError::TomlSeError(e) => write!(f, "Configuration serialization error: {}", e),
            ConfigError::GenericError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::IoError(e) => Some(e),
            ConfigError::TomlDeError(e) => Some(e),
            ConfigError::TomlSeError(e) => Some(e),
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
                toml::from_str(&contents).map_err(ConfigError::TomlDeError)
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                warn!("Configuration file not found at {}. Using default configuration.", path.display());
                Ok(Config::default_config())
            }
            Err(e) => {
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
             if let Err(e) = fs::create_dir_all(parent) {
                  error!("Failed to create parent directory for config file {}: {}", path.display(), e);
                  return Err(ConfigError::IoError(e));
             }
         } else {
              warn!("Config path {} has no parent directory. Saving to root?", path.display());
         }


        let toml_string = toml::to_string_pretty(self).map_err(ConfigError::TomlSeError)?;
        debug!("Saving config contents:\n{}", toml_string);

        let mut file = fs::File::create(path).map_err(ConfigError::IoError)?;
        file.write_all(toml_string.as_bytes()).map_err(ConfigError::IoError)?;

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
}

// Test code (add necessary dependencies like tempfile)
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir; // Add tempfile = "3.2" to Cargo.toml

    #[test]
    fn test_default_config() {
        let config = Config::default_config();
        assert_eq!(config.game_paths.len(), 0);
        assert_eq!(config.input_mappings, vec!["Auto-detect".to_string(), "Auto-detect".to_string()]);
        assert_eq!(config.window_layout, "horizontal".to_string());
        assert_eq!(config.network_ports, vec![7777, 7778]);
        assert_eq!(config.use_proton, false);
    }

    #[test]
    fn test_save_and_load_config() {
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

    // TODO: Add test for invalid TOML format
}
