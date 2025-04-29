use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use toml;
use log::{error, info};
use std::path::Path; // Import Path
use std::error::Error; // Import Error trait

// Custom error type for configuration operations
#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    TomlDeError(toml::de::Error), // Deserialization error
    TomlSeError(toml::ser::Error), // Serialization error
    GenericError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "Configuration I/O error: {}", e),
            ConfigError::TomlDeError(e) => write!(f, "Configuration parsing error: {}", e),
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


#[derive(Serialize, Deserialize, Debug, PartialEq)] // Derive PartialEq for easier testing
pub struct Config {
    pub game_paths: Vec<PathBuf>, // Use PathBuf for paths
    pub input_mappings: Vec<String>, // Or a more structured type for mappings
    pub window_layout: String, // Consider an enum for layout options
    pub network_ports: Vec<u16>,
    // Add other configuration fields as needed (e.g., debug logging level, monitor assignments)
}

impl Config {
    /// Loads the configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - A Path to the configuration file.
    ///
    /// # Returns
    ///
    /// * `Result<Config, ConfigError>` - Returns the configuration if successful, otherwise returns a ConfigError.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        info!("Loading configuration from {}", path.display());
        let contents = fs::read_to_string(path).map_err(ConfigError::IoError)?; // Map IO error
        let config: Config = toml::from_str(&contents).map_err(ConfigError::TomlDeError)?; // Map TOML deserialization error
        info!("Configuration loaded successfully.");
        Ok(config)
    }

    /// Saves the configuration to a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - A Path to the configuration file.
    ///
    /// # Returns
    ///
    /// * `Result<(), ConfigError>` - Returns Ok if successful, otherwise returns a ConfigError.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        info!("Saving configuration to {}", path.display());
        let toml_string = toml::to_string(self).map_err(ConfigError::TomlSeError)?; // Map TOML serialization error
        let mut file = fs::File::create(path).map_err(ConfigError::IoError)?; // Map IO error
        file.write_all(toml_string.as_bytes()).map_err(ConfigError::IoError)?; // Map IO error
        info!("Configuration saved successfully.");
        Ok(())
    }

    // You might want a function to get default configuration
    pub fn default_config() -> Self {
        Config {
            game_paths: vec![],
            input_mappings: vec![],
            window_layout: "horizontal".to_string(), // Provide a default layout
            network_ports: vec![],
             // Set default values for other fields
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir; // Add tempfile = "3.2" to your Cargo.toml

    #[test]
    fn test_load_save_config() {
        // Create a temporary directory for test files
        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let temp_config_path = temp_test_dir.path().join("test_config.toml");

        let config = Config {
            game_paths: vec![PathBuf::from("path/to/game1"), PathBuf::from("path/to/game2")],
            input_mappings: vec!["key1=action1".to_string(), "key2=action2".to_string()],
            window_layout: "layout1".to_string(),
            network_ports: vec![8080, 8081],
        };

        // Save the configuration to the temporary file
        config.save(&temp_config_path).expect("Failed to save test config");

        // Load the configuration from the temporary file
        let loaded_config = Config::load(&temp_config_path).expect("Failed to load test config");

        // Assert that the loaded config matches the original config
        assert_eq!(config, loaded_config);

        // temp_test_dir and its contents are automatically cleaned up when it goes out of scope
    }

    #[test]
    fn test_load_invalid_config_format() {
        let invalid_toml = r#"
        game_paths = ["path/to/game1"]
        input_mappings = ["key1=action1"]
        window_layout = "layout1"
        network_ports = [8080, "invalid_port"] # This should be a number, not a string
        "#;

        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let temp_invalid_config_path = temp_test_dir.path().join("invalid_config.toml");

        // Write invalid TOML to a temporary file
        fs::write(&temp_invalid_config_path, invalid_toml).expect("Failed to write invalid test config");

        // Attempt to load the invalid configuration
        let result = Config::load(&temp_invalid_config_path);

        // Assert that loading failed with a ConfigError (specifically a TomlDeError)
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::TomlDeError(_) => {
                info!("Successfully caught TOML deserialization error for invalid config.");
            }
            other_error => {
                panic!("Expected TomlDeError but got: {:?}", other_error);
            }
        }

        // temp_test_dir and its contents are automatically cleaned up
    }

     #[test]
     fn test_load_nonexistent_config() {
         let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
         let non_existent_path = temp_test_dir.path().join("non_existent_config.toml");

         // Attempt to load a non-existent file
         let result = Config::load(&non_existent_path);

         // Assert that loading failed with an IoError (specifically NotFound)
         assert!(result.is_err());
         match result.unwrap_err() {
             ConfigError::IoError(io_err) => {
                 assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
                 info!("Successfully caught IoError::NotFound for non-existent config.");
             }
             other_error => {
                  panic!("Expected IoError::NotFound but got: {:?}", other_error);
             }
         }
     }

     #[test]
    fn test_default_config() {
        let default = Config::default_config();
        assert_eq!(default.game_paths, vec![]);
        assert_eq!(default.input_mappings, vec![]);
        assert_eq!(default.window_layout, "horizontal".to_string());
        assert_eq!(default.network_ports, vec![]);
    }
}
