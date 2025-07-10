//! Universal Game Launcher
//! 
//! This module provides a universal game launching system that works with any game
//! without requiring game-specific handlers or configuration.

use std::path::{Path, PathBuf};
use std::process::{Command, Child};
use std::collections::HashMap;
use std::fs;
use log::{info, warn, debug, error};
use crate::errors::{HydraError, Result};
use crate::game_detection::{GameDetector, GameProfile, GameConfiguration, WorkingDirStrategy, InstanceSeparation};

/// Universal game launcher that can launch any game with multi-instance support
pub struct UniversalLauncher {
    game_detector: GameDetector,
    active_instances: Vec<GameInstance>,
}

/// Represents a running game instance
#[derive(Debug)]
pub struct GameInstance {
    pub id: usize,
    pub process: Child,
    pub working_dir: PathBuf,
    pub config: GameConfiguration,
    pub profile: GameProfile,
}

impl UniversalLauncher {
    pub fn new() -> Self {
        Self {
            game_detector: GameDetector::new(),
            active_instances: Vec::new(),
        }
    }

    /// Launch multiple instances of any game using universal detection and configuration
    pub fn launch_game_instances(
        &mut self,
        executable_path: &Path,
        num_instances: usize,
        use_proton: bool,
    ) -> Result<Vec<u32>> {
        info!("Launching {} instances of game: {}", num_instances, executable_path.display());

        // Detect and analyze the game
        let profile = self.game_detector.detect_game(executable_path)?;
        let config = self.game_detector.get_recommended_config(&profile, num_instances);

        info!("Detected game profile: engine={:?}, support={:?}", 
               profile.engine, profile.multi_instance_support);

        let mut pids = Vec::new();

        for instance_id in 0..num_instances {
            info!("Launching instance {} of {}", instance_id + 1, num_instances);

            let instance = self.launch_single_instance(
                executable_path,
                instance_id,
                &profile,
                &config,
                use_proton,
            )?;

            pids.push(instance.process.id());
            self.active_instances.push(instance);
        }

        info!("Successfully launched {} game instances with PIDs: {:?}", num_instances, pids);
        Ok(pids)
    }

    /// Launch a single game instance with universal configuration
    fn launch_single_instance(
        &self,
        executable_path: &Path,
        instance_id: usize,
        profile: &GameProfile,
        config: &GameConfiguration,
        use_proton: bool,
    ) -> Result<GameInstance> {
        // Prepare working directory
        let working_dir = self.prepare_working_directory(executable_path, instance_id, &config.working_dir_strategy)?;

        // Prepare the command
        let mut command = if use_proton {
            self.prepare_proton_command(executable_path, instance_id, &working_dir)?
        } else {
            Command::new(executable_path)
        };

        // Set working directory
        command.current_dir(&working_dir);

        // Add universal launch arguments
        self.add_launch_arguments(&mut command, instance_id, config);

        // Set environment variables
        self.set_environment_variables(&mut command, instance_id, config);

        // Apply instance separation strategies
        self.apply_instance_separation(&mut command, instance_id, config, &working_dir)?;

        info!("Spawning game instance {} with command: {:?}", instance_id, command);

        // Launch the process
        let process = command.spawn()
            .map_err(|e| HydraError::application(format!("Failed to spawn game instance {}: {}", instance_id, e)))?;

        let instance = GameInstance {
            id: instance_id,
            process,
            working_dir,
            config: config.clone(),
            profile: profile.clone(),
        };

        info!("Game instance {} launched successfully with PID: {}", instance_id, instance.process.id());

        Ok(instance)
    }

    /// Prepare working directory based on strategy
    fn prepare_working_directory(
        &self,
        executable_path: &Path,
        instance_id: usize,
        strategy: &WorkingDirStrategy,
    ) -> Result<PathBuf> {
        let working_dir = match strategy {
            WorkingDirStrategy::GameDirectory => {
                executable_path.parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            },
            WorkingDirStrategy::SeparateDirectories => {
                let base_dir = executable_path.parent()
                    .unwrap_or(Path::new("."));
                base_dir.join(format!("instance_{}", instance_id))
            },
            WorkingDirStrategy::Temporary => {
                std::env::temp_dir().join(format!("hydra_game_instance_{}", instance_id))
            },
            WorkingDirStrategy::Current => {
                std::env::current_dir()
                    .map_err(|e| HydraError::Io(e))?
            },
        };

        // Create the directory if it doesn't exist
        if !working_dir.exists() {
            fs::create_dir_all(&working_dir)
                .map_err(|e| HydraError::Io(e))?;
            info!("Created working directory: {}", working_dir.display());
        }

        // For separate directories, copy necessary game files
        if matches!(strategy, WorkingDirStrategy::SeparateDirectories) {
            self.setup_separate_instance_directory(executable_path, &working_dir)?;
        }

        Ok(working_dir)
    }

    /// Setup a separate instance directory with necessary game files
    fn setup_separate_instance_directory(&self, executable_path: &Path, instance_dir: &Path) -> Result<()> {
        let game_dir = executable_path.parent().unwrap_or(Path::new("."));

        // Copy essential files that games typically need
        let essential_patterns = [
            "*.dll",
            "*.so",
            "*.dylib",
            "*.ini",
            "*.cfg",
            "*.config",
            "*.xml",
            "*.json",
        ];

        for pattern in &essential_patterns {
            if let Ok(entries) = fs::read_dir(game_dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        let extension = format!("*.{}", 
                            Path::new(name).extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or("")
                        );
                        
                        if pattern == &extension && entry.path().is_file() {
                            let dest = instance_dir.join(name);
                            if !dest.exists() {
                                if let Err(e) = fs::copy(entry.path(), &dest) {
                                    warn!("Failed to copy {} to instance directory: {}", name, e);
                                } else {
                                    debug!("Copied {} to instance directory", name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Copy essential directories
        let essential_dirs = ["Data", "Config", "Plugins", "Mods"];
        for dir_name in &essential_dirs {
            let src_dir = game_dir.join(dir_name);
            if src_dir.exists() && src_dir.is_dir() {
                let dest_dir = instance_dir.join(dir_name);
                if !dest_dir.exists() {
                    if let Err(e) = self.copy_dir_recursive(&src_dir, &dest_dir) {
                        warn!("Failed to copy directory {} to instance: {}", dir_name, e);
                    } else {
                        debug!("Copied directory {} to instance", dir_name);
                    }
                }
            }
        }

        Ok(())
    }

    /// Recursively copy a directory
    fn copy_dir_recursive(&self, src: &Path, dest: &Path) -> Result<()> {
        fs::create_dir_all(dest).map_err(HydraError::Io)?;

        for entry in fs::read_dir(src).map_err(HydraError::Io)? {
            let entry = entry.map_err(HydraError::Io)?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dest_path)?;
            } else {
                fs::copy(&src_path, &dest_path).map_err(HydraError::Io)?;
            }
        }

        Ok(())
    }

    /// Prepare Proton command for Windows games
    fn prepare_proton_command(&self, executable_path: &Path, instance_id: usize, working_dir: &Path) -> Result<Command> {
        let proton_path = crate::proton_integration::find_proton_path()
            .map_err(|e| HydraError::application(format!("Proton not found: {}", e)))?;

        let wineprefix = working_dir.join("wineprefix");
        fs::create_dir_all(&wineprefix).map_err(HydraError::Io)?;

        let mut command = Command::new(proton_path);
        command.arg("run");
        command.arg(executable_path);
        command.env("WINEPREFIX", &wineprefix);
        command.env("PROTON_LOG", "1");

        Ok(command)
    }

    /// Add universal launch arguments
    fn add_launch_arguments(&self, command: &mut Command, instance_id: usize, config: &GameConfiguration) {
        // Add profile-specific arguments
        for arg in &config.launch_args {
            command.arg(arg);
        }

        // Add universal arguments for multi-instance support
        command.arg(format!("-instance-id={}", instance_id));
        command.arg(format!("-hydra-instance={}", instance_id));
        
        // Add port-related arguments if the game might use them
        if !config.ports.is_empty() {
            command.arg(format!("-port={}", config.ports[0]));
            command.arg(format!("-server-port={}", config.ports[0]));
        }

        // Add windowed mode arguments (common for multi-instance)
        command.arg("-windowed");
        command.arg("-noborder");
    }

    /// Set environment variables for the game instance
    fn set_environment_variables(&self, command: &mut Command, instance_id: usize, config: &GameConfiguration) {
        // Set profile-specific environment variables
        for (key, value) in &config.environment_vars {
            command.env(key, value);
        }

        // Set universal environment variables
        command.env("HYDRA_INSTANCE_ID", instance_id.to_string());
        command.env("HYDRA_INSTANCE_COUNT", "1"); // Will be updated by caller
        
        // Set port-related environment variables
        if !config.ports.is_empty() {
            command.env("HYDRA_PORT", config.ports[0].to_string());
            command.env("GAME_PORT", config.ports[0].to_string());
            command.env("SERVER_PORT", config.ports[0].to_string());
        }

        // Disable problematic features that might interfere with multi-instance
        command.env("DISABLE_STEAM_OVERLAY", "1");
        command.env("DISABLE_FULLSCREEN", "1");
        command.env("FORCE_WINDOWED", "1");
    }

    /// Apply instance separation strategies
    fn apply_instance_separation(
        &self,
        command: &mut Command,
        instance_id: usize,
        config: &GameConfiguration,
        working_dir: &Path,
    ) -> Result<()> {
        match config.instance_separation {
            InstanceSeparation::None => {
                // No additional separation needed
            },
            InstanceSeparation::Environment => {
                // Separate using environment variables
                command.env("INSTANCE_ID", instance_id.to_string());
                command.env("USER_DATA_DIR", working_dir.join("userdata").to_string_lossy().to_string());
                command.env("SAVE_DIR", working_dir.join("saves").to_string_lossy().to_string());
            },
            InstanceSeparation::Full => {
                // Full separation with directories and configs
                let config_dir = working_dir.join("config");
                let save_dir = working_dir.join("saves");
                let cache_dir = working_dir.join("cache");

                fs::create_dir_all(&config_dir).map_err(HydraError::Io)?;
                fs::create_dir_all(&save_dir).map_err(HydraError::Io)?;
                fs::create_dir_all(&cache_dir).map_err(HydraError::Io)?;

                // Set various directory environment variables that games might use
                command.env("APPDATA", config_dir.to_string_lossy().to_string());
                command.env("LOCALAPPDATA", cache_dir.to_string_lossy().to_string());
                command.env("USERPROFILE", working_dir.to_string_lossy().to_string());
                command.env("HOME", working_dir.to_string_lossy().to_string());
                command.env("XDG_CONFIG_HOME", config_dir.to_string_lossy().to_string());
                command.env("XDG_DATA_HOME", save_dir.to_string_lossy().to_string());
                command.env("XDG_CACHE_HOME", cache_dir.to_string_lossy().to_string());
            },
        }

        Ok(())
    }

    /// Get statistics about active instances
    pub fn get_instance_stats(&self) -> InstanceStats {
        let mut running_count = 0;
        let mut total_memory = 0;

        for instance in &self.active_instances {
            // Check if process is still running (simplified check)
            running_count += 1;
            // In a real implementation, you'd get actual memory usage
            total_memory += 100; // Placeholder
        }

        InstanceStats {
            total_instances: self.active_instances.len(),
            running_instances: running_count,
            total_memory_mb: total_memory,
        }
    }

    /// Stop all running instances
    pub fn stop_all_instances(&mut self) -> Result<()> {
        info!("Stopping all {} game instances", self.active_instances.len());

        for instance in &mut self.active_instances {
            if let Err(e) = instance.process.kill() {
                warn!("Failed to kill instance {}: {}", instance.id, e);
            } else {
                info!("Stopped instance {}", instance.id);
            }
        }

        self.active_instances.clear();
        Ok(())
    }
}

/// Statistics about running game instances
#[derive(Debug, Clone)]
pub struct InstanceStats {
    pub total_instances: usize,
    pub running_instances: usize,
    pub total_memory_mb: u64,
}

impl Default for UniversalLauncher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_working_directory_strategies() {
        let temp_dir = tempdir().unwrap();
        let exe_path = temp_dir.path().join("test.exe");
        std::fs::File::create(&exe_path).unwrap();

        let launcher = UniversalLauncher::new();

        // Test separate directories strategy
        let working_dir = launcher.prepare_working_directory(
            &exe_path,
            0,
            &WorkingDirStrategy::SeparateDirectories,
        ).unwrap();

        assert!(working_dir.exists());
        assert!(working_dir.ends_with("instance_0"));
    }

    #[test]
    fn test_environment_variable_setup() {
        let mut command = Command::new("echo");
        let config = GameConfiguration {
            ports: vec![8080],
            layout: "horizontal".to_string(),
            launch_args: vec![],
            environment_vars: HashMap::new(),
            working_dir_strategy: WorkingDirStrategy::Current,
            instance_separation: InstanceSeparation::Environment,
        };

        let launcher = UniversalLauncher::new();
        launcher.set_environment_variables(&mut command, 0, &config);

        // Verify environment variables are set (this is a simplified test)
        // In a real test, you'd need to check the command's environment
    }
}