//! Universal Game Detection and Configuration System
//! 
//! This module provides automatic game detection and universal configuration
//! that works with any game without requiring game-specific handlers.

use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{info, warn, debug, error};
use crate::errors::{HydraError, Result};

/// Universal game profile that can be applied to any game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProfile {
    /// Game executable name or pattern
    pub executable_pattern: String,
    /// Detected game engine (if any)
    pub engine: Option<GameEngine>,
    /// Recommended network ports for this type of game
    pub default_ports: Vec<u16>,
    /// Recommended window layout
    pub default_layout: String,
    /// Whether the game likely supports multiple instances
    pub multi_instance_support: MultiInstanceSupport,
    /// Launch arguments that help with multi-instance support
    pub launch_args: Vec<String>,
    /// Environment variables to set
    pub environment_vars: HashMap<String, String>,
    /// Working directory strategy
    pub working_dir_strategy: WorkingDirStrategy,
}

/// Detected game engine types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEngine {
    Unity,
    UnrealEngine,
    Godot,
    GameMaker,
    Construct,
    Custom(String),
    Unknown,
}

/// Multi-instance support levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiInstanceSupport {
    /// Game natively supports multiple instances
    Native,
    /// Game can run multiple instances with some configuration
    Configurable,
    /// Game requires workarounds for multiple instances
    RequiresWorkarounds,
    /// Game likely won't work with multiple instances
    Unsupported,
}

/// Working directory strategies for different games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkingDirStrategy {
    /// Use the game's installation directory
    GameDirectory,
    /// Create separate directories for each instance
    SeparateDirectories,
    /// Use a temporary directory
    Temporary,
    /// Use the current working directory
    Current,
}

/// Universal game detector that analyzes games without specific handlers
pub struct GameDetector {
    /// Cache of detected game profiles
    profile_cache: HashMap<PathBuf, GameProfile>,
}

impl GameDetector {
    pub fn new() -> Self {
        Self {
            profile_cache: HashMap::new(),
        }
    }

    /// Detect and analyze a game executable to create a universal profile
    pub fn detect_game(&mut self, executable_path: &Path) -> Result<GameProfile> {
        // Check cache first
        if let Some(cached_profile) = self.profile_cache.get(executable_path) {
            debug!("Using cached profile for {}", executable_path.display());
            return Ok(cached_profile.clone());
        }

        info!("Analyzing game executable: {}", executable_path.display());

        let mut profile = GameProfile {
            executable_pattern: executable_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            engine: None,
            default_ports: vec![7777, 7778, 7779, 7780], // Common game ports
            default_layout: "horizontal".to_string(),
            multi_instance_support: MultiInstanceSupport::Configurable,
            launch_args: Vec::new(),
            environment_vars: HashMap::new(),
            working_dir_strategy: WorkingDirStrategy::SeparateDirectories,
        };

        // Detect game engine
        profile.engine = self.detect_engine(executable_path)?;

        // Configure based on detected engine
        self.configure_for_engine(&mut profile);

        // Analyze executable for additional hints
        self.analyze_executable(&mut profile, executable_path)?;

        // Cache the profile
        self.profile_cache.insert(executable_path.to_path_buf(), profile.clone());

        info!("Generated universal profile for {}: engine={:?}, support={:?}", 
               executable_path.display(), profile.engine, profile.multi_instance_support);

        Ok(profile)
    }

    /// Detect the game engine by analyzing the executable and its directory
    fn detect_engine(&self, executable_path: &Path) -> Result<Option<GameEngine>> {
        let game_dir = executable_path.parent().unwrap_or(Path::new("."));
        
        // Check for engine-specific files and directories
        if self.check_unity_indicators(game_dir) {
            return Ok(Some(GameEngine::Unity));
        }
        
        if self.check_unreal_indicators(game_dir) {
            return Ok(Some(GameEngine::UnrealEngine));
        }
        
        if self.check_godot_indicators(game_dir) {
            return Ok(Some(GameEngine::Godot));
        }
        
        if self.check_gamemaker_indicators(game_dir) {
            return Ok(Some(GameEngine::GameMaker));
        }

        // Check executable name patterns
        let exe_name = executable_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if exe_name.contains("unity") {
            return Ok(Some(GameEngine::Unity));
        }
        
        if exe_name.contains("unreal") || exe_name.contains("ue4") || exe_name.contains("ue5") {
            return Ok(Some(GameEngine::UnrealEngine));
        }

        Ok(Some(GameEngine::Unknown))
    }

    /// Check for Unity engine indicators
    fn check_unity_indicators(&self, game_dir: &Path) -> bool {
        let unity_indicators = [
            "UnityPlayer.dll",
            "UnityCrashHandler64.exe",
            "UnityCrashHandler32.exe",
            "Managed",
            "MonoBleedingEdge",
            "*_Data", // Unity data folder pattern
        ];

        for indicator in &unity_indicators {
            if indicator.contains('*') {
                // Pattern matching for data folders
                if let Ok(entries) = fs::read_dir(game_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with("_Data") && entry.path().is_dir() {
                                return true;
                            }
                        }
                    }
                }
            } else if game_dir.join(indicator).exists() {
                return true;
            }
        }
        false
    }

    /// Check for Unreal Engine indicators
    fn check_unreal_indicators(&self, game_dir: &Path) -> bool {
        let unreal_indicators = [
            "Engine",
            "Content",
            "Binaries",
            "Config",
            "Saved",
            "*.pak", // Unreal asset packages
        ];

        for indicator in &unreal_indicators {
            if indicator.contains('*') {
                if let Ok(entries) = fs::read_dir(game_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".pak") {
                                return true;
                            }
                        }
                    }
                }
            } else if game_dir.join(indicator).exists() {
                return true;
            }
        }
        false
    }

    /// Check for Godot engine indicators
    fn check_godot_indicators(&self, game_dir: &Path) -> bool {
        let godot_indicators = [
            "project.godot",
            ".godot",
            "*.pck", // Godot package files
        ];

        for indicator in &godot_indicators {
            if indicator.contains('*') {
                if let Ok(entries) = fs::read_dir(game_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".pck") {
                                return true;
                            }
                        }
                    }
                }
            } else if game_dir.join(indicator).exists() {
                return true;
            }
        }
        false
    }

    /// Check for GameMaker indicators
    fn check_gamemaker_indicators(&self, game_dir: &Path) -> bool {
        let gamemaker_indicators = [
            "data.win",
            "game.ios",
            "game.droid",
            "audiogroup*.dat",
        ];

        for indicator in &gamemaker_indicators {
            if indicator.contains('*') {
                if let Ok(entries) = fs::read_dir(game_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with("audiogroup") && name.ends_with(".dat") {
                                return true;
                            }
                        }
                    }
                }
            } else if game_dir.join(indicator).exists() {
                return true;
            }
        }
        false
    }

    /// Configure profile based on detected engine
    fn configure_for_engine(&self, profile: &mut GameProfile) {
        match profile.engine {
            Some(GameEngine::Unity) => {
                profile.multi_instance_support = MultiInstanceSupport::Configurable;
                profile.launch_args = vec![
                    "-force-opengl".to_string(), // Better compatibility
                    "-screen-fullscreen".to_string(),
                    "0".to_string(), // Windowed mode
                ];
                profile.environment_vars.insert("UNITY_MIXED_CALLSTACK".to_string(), "1".to_string());
                profile.working_dir_strategy = WorkingDirStrategy::SeparateDirectories;
            },
            Some(GameEngine::UnrealEngine) => {
                profile.multi_instance_support = MultiInstanceSupport::Configurable;
                profile.launch_args = vec![
                    "-windowed".to_string(),
                    "-ResX=800".to_string(),
                    "-ResY=600".to_string(),
                ];
                profile.working_dir_strategy = WorkingDirStrategy::SeparateDirectories;
            },
            Some(GameEngine::Godot) => {
                profile.multi_instance_support = MultiInstanceSupport::Native;
                profile.launch_args = vec![
                    "--windowed".to_string(),
                ];
                profile.working_dir_strategy = WorkingDirStrategy::GameDirectory;
            },
            Some(GameEngine::GameMaker) => {
                profile.multi_instance_support = MultiInstanceSupport::RequiresWorkarounds;
                profile.working_dir_strategy = WorkingDirStrategy::SeparateDirectories;
            },
            _ => {
                // Default configuration for unknown engines
                profile.multi_instance_support = MultiInstanceSupport::Configurable;
                profile.working_dir_strategy = WorkingDirStrategy::SeparateDirectories;
            }
        }
    }

    /// Analyze executable for additional configuration hints
    fn analyze_executable(&self, profile: &mut GameProfile, executable_path: &Path) -> std::result::Result<(), GameDetectionError> {
        // Check if it's a Windows executable
        if crate::proton_integration::is_windows_binary(executable_path)
            .map_err(|e| GameDetectionError::AnalysisFailed(e.to_string()))?
        {
            profile.environment_vars.insert("WINEDEBUG".to_string(), "-all".to_string());
            profile.working_dir_strategy = WorkingDirStrategy::SeparateDirectories;
        }

        // Analyze file size for hints about game complexity
        let metadata = fs::metadata(executable_path)
            .map_err(GameDetectionError::Io)?;
        {
            let size_mb = metadata.len() / (1024 * 1024);
            
            if size_mb > 100 {
                // Large executable, likely needs more resources
                profile.environment_vars.insert("HYDRA_LARGE_GAME".to_string(), "1".to_string());
            }
        }

        Ok(())
    }

    /// Get recommended configuration for a game
    pub fn get_recommended_config(&self, profile: &GameProfile, num_instances: usize) -> GameConfiguration {
        let mut ports = profile.default_ports.clone();
        
        // Ensure we have enough ports for all instances
        while ports.len() < num_instances {
            let next_port = ports.last().unwrap_or(&7777) + 1;
            ports.push(next_port);
        }

        GameConfiguration {
            ports: ports.into_iter().take(num_instances).collect(),
            layout: profile.default_layout.clone(),
            launch_args: profile.launch_args.clone(),
            environment_vars: profile.environment_vars.clone(),
            working_dir_strategy: profile.working_dir_strategy.clone(),
            instance_separation: match profile.multi_instance_support {
                MultiInstanceSupport::Native => InstanceSeparation::None,
                MultiInstanceSupport::Configurable => InstanceSeparation::Environment,
                MultiInstanceSupport::RequiresWorkarounds => InstanceSeparation::Full,
                MultiInstanceSupport::Unsupported => InstanceSeparation::Full,
            },
        }
    }
}

/// Configuration generated for a specific game and instance count
#[derive(Debug, Clone)]
pub struct GameConfiguration {
    pub ports: Vec<u16>,
    pub layout: String,
    pub launch_args: Vec<String>,
    pub environment_vars: HashMap<String, String>,
    pub working_dir_strategy: WorkingDirStrategy,
    pub instance_separation: InstanceSeparation,
}

/// Strategies for separating game instances
#[derive(Debug, Clone)]
pub enum InstanceSeparation {
    /// No separation needed (game handles it natively)
    None,
    /// Separate using environment variables only
    Environment,
    /// Full separation (directories, configs, etc.)
    Full,
}

impl Default for GameDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_unity_detection() {
        let temp_dir = tempdir().unwrap();
        let game_dir = temp_dir.path();
        
        // Create Unity indicators
        fs::create_dir_all(game_dir.join("TestGame_Data")).unwrap();
        fs::File::create(game_dir.join("UnityPlayer.dll")).unwrap();
        
        let detector = GameDetector::new();
        assert!(detector.check_unity_indicators(game_dir));
    }

    #[test]
    fn test_game_profile_generation() {
        let temp_dir = tempdir().unwrap();
        let exe_path = temp_dir.path().join("TestGame.exe");
        fs::File::create(&exe_path).unwrap();
        
        let mut detector = GameDetector::new();
        let profile = detector.detect_game(&exe_path).unwrap();
        
        assert_eq!(profile.executable_pattern, "TestGame.exe");
        assert!(!profile.default_ports.is_empty());
    }
}