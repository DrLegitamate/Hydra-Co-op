use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use toml;
use log::{error, info};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    game_paths: Vec<String>,
    input_mappings: Vec<String>,
    window_layout: String,
    network_ports: Vec<u16>,
}

impl Config {
    /// Loads the configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - A string slice that holds the path to the configuration file.
    ///
    /// # Returns
    ///
    /// * `Result<Config, io::Error>` - Returns the configuration if successful, otherwise returns an IO error.
    pub fn load(path: &str) -> Result<Self, io::Error> {
        info!("Loading configuration from {}", path);
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents).map_err(|e| {
            error!("Failed to parse configuration: {}", e);
            io::Error::new(io::ErrorKind::InvalidData, e)
        })?;
        Ok(config)
    }

    /// Saves the configuration to a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - A string slice that holds the path to the configuration file.
    ///
    /// # Returns
    ///
    /// * `Result<(), io::Error>` - Returns Ok if successful, otherwise returns an IO error.
    pub fn save(&self, path: &str) -> Result<(), io::Error> {
        info!("Saving configuration to {}", path);
        let toml_string = toml::to_string(self).map_err(|e| {
            error!("Failed to serialize configuration: {}", e);
            io::Error::new(io::ErrorKind::InvalidData, e)
        })?;
        let mut file = fs::File::create(path)?;
        file.write_all(toml_string.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_save_config() {
        let config = Config {
            game_paths: vec!["path/to/game1".to_string(), "path/to/game2".to_string()],
            input_mappings: vec!["key1=action1".to_string(), "key2=action2".to_string()],
            window_layout: "layout1".to_string(),
            network_ports: vec![8080, 8081],
        };

        let temp_path = "/tmp/test_config.toml";
        config.save(temp_path).unwrap();
        let loaded_config = Config::load(temp_path).unwrap();

        assert_eq!(config.game_paths, loaded_config.game_paths);
        assert_eq!(config.input_mappings, loaded_config.input_mappings);
        assert_eq!(config.window_layout, loaded_config.window_layout);
        assert_eq!(config.network_ports, loaded_config.network_ports);

        fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn test_load_invalid_config() {
        let invalid_toml = r#"
        game_paths = ["path/to/game1"]
        input_mappings = ["key1=action1"]
        window_layout = "layout1"
        network_ports = [8080, "invalid_port"]
        "#;

        let temp_path = "/tmp/invalid_config.toml";
        fs::write(temp_path, invalid_toml).unwrap();

        let result = Config::load(temp_path);
        assert!(result.is_err());

        fs::remove_file(temp_path).unwrap();
    }
}
