//! Adaptive Configuration System
//! 
//! This module provides runtime adaptation and learning capabilities
//! to improve game compatibility automatically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use log::{info, warn, debug};
use crate::errors::{HydraError, Result};
use crate::game_detection::{GameProfile, GameEngine};

/// Adaptive configuration that learns from successful game launches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    /// Game-specific adaptations learned over time
    pub game_adaptations: HashMap<String, GameAdaptation>,
    /// Global success patterns
    pub success_patterns: Vec<SuccessPattern>,
    /// Failed configuration attempts to avoid
    pub failed_configs: Vec<FailedConfig>,
}

/// Adaptation data for a specific game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAdaptation {
    /// Game executable hash or identifier
    pub game_id: String,
    /// Number of successful launches with this config
    pub success_count: u32,
    /// Last successful launch time
    pub last_success: SystemTime,
    /// Optimal configuration found through testing
    pub optimal_config: OptimalConfig,
    /// Known working launch arguments
    pub working_args: Vec<String>,
    /// Known working environment variables
    pub working_env_vars: HashMap<String, String>,
    /// Compatibility notes
    pub notes: Vec<String>,
}

/// Optimal configuration discovered for a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimalConfig {
    /// Best working directory strategy
    pub working_dir_strategy: String,
    /// Optimal instance separation level
    pub separation_level: String,
    /// Best network ports
    pub ports: Vec<u16>,
    /// Optimal window layout
    pub layout: String,
    /// Required launch delay between instances
    pub launch_delay_ms: u64,
}

/// Pattern that led to successful multi-instance launch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessPattern {
    /// Game engine type
    pub engine: Option<String>,
    /// Configuration that worked
    pub config: HashMap<String, String>,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Number of times this pattern succeeded
    pub success_count: u32,
}

/// Configuration that failed to work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedConfig {
    /// Game identifier
    pub game_id: String,
    /// Configuration that failed
    pub config: HashMap<String, String>,
    /// Error message or reason for failure
    pub failure_reason: String,
    /// Timestamp of failure
    pub failed_at: SystemTime,
}

/// Adaptive configuration manager
pub struct AdaptiveConfigManager {
    config: AdaptiveConfig,
    config_path: PathBuf,
}

impl AdaptiveConfigManager {
    /// Create a new adaptive configuration manager
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            AdaptiveConfig::default()
        };

        Ok(Self {
            config,
            config_path,
        })
    }

    /// Load adaptive configuration from file
    fn load_config(path: &Path) -> Result<AdaptiveConfig> {
        let content = std::fs::read_to_string(path)
            .map_err(HydraError::Io)?;
        
        toml::from_str(&content)
            .map_err(|e| HydraError::application(format!("Failed to parse adaptive config: {}", e)))
    }

    /// Save adaptive configuration to file
    pub fn save_config(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.config)
            .map_err(|e| HydraError::application(format!("Failed to serialize adaptive config: {}", e)))?;

        std::fs::write(&self.config_path, content)
            .map_err(HydraError::Io)?;

        debug!("Saved adaptive configuration to {}", self.config_path.display());
        Ok(())
    }

    /// Get adaptive configuration for a specific game
    pub fn get_game_adaptation(&self, game_id: &str) -> Option<&GameAdaptation> {
        self.config.game_adaptations.get(game_id)
    }

    /// Record a successful game launch configuration
    pub fn record_success(
        &mut self,
        game_id: String,
        profile: &GameProfile,
        config: &crate::game_detection::GameConfiguration,
        launch_time: Duration,
    ) -> Result<()> {
        info!("Recording successful launch for game: {}", game_id);

        // Update or create game adaptation
        let adaptation = self.config.game_adaptations
            .entry(game_id.clone())
            .or_insert_with(|| GameAdaptation {
                game_id: game_id.clone(),
                success_count: 0,
                last_success: SystemTime::now(),
                optimal_config: OptimalConfig {
                    working_dir_strategy: format!("{:?}", config.working_dir_strategy),
                    separation_level: format!("{:?}", config.instance_separation),
                    ports: config.ports.clone(),
                    layout: config.layout.clone(),
                    launch_delay_ms: 0,
                },
                working_args: config.launch_args.clone(),
                working_env_vars: config.environment_vars.clone(),
                notes: Vec::new(),
            });

        adaptation.success_count += 1;
        adaptation.last_success = SystemTime::now();

        // Update optimal config if this launch was faster or more successful
        if launch_time.as_millis() < adaptation.optimal_config.launch_delay_ms as u128 {
            adaptation.optimal_config.launch_delay_ms = launch_time.as_millis() as u64;
        }

        // Record success pattern
        let mut pattern_config = HashMap::new();
        pattern_config.insert("working_dir".to_string(), format!("{:?}", config.working_dir_strategy));
        pattern_config.insert("separation".to_string(), format!("{:?}", config.instance_separation));
        pattern_config.insert("layout".to_string(), config.layout.clone());

        let engine_str = profile.engine.as_ref().map(|e| format!("{:?}", e));
        
        // Find existing pattern or create new one
        let mut found_pattern = false;
        for pattern in &mut self.config.success_patterns {
            if pattern.engine == engine_str && pattern.config == pattern_config {
                pattern.success_count += 1;
                pattern.success_rate = (pattern.success_rate * (pattern.success_count - 1) as f64 + 1.0) / pattern.success_count as f64;
                found_pattern = true;
                break;
            }
        }

        if !found_pattern {
            self.config.success_patterns.push(SuccessPattern {
                engine: engine_str,
                config: pattern_config,
                success_rate: 1.0,
                success_count: 1,
            });
        }

        self.save_config()?;
        Ok(())
    }

    /// Record a failed game launch configuration
    pub fn record_failure(
        &mut self,
        game_id: String,
        config: &crate::game_detection::GameConfiguration,
        error: &str,
    ) -> std::result::Result<(), AdaptiveConfigError> {
        warn!("Recording failed launch for game: {} - {}", game_id, error);

        let mut failed_config = HashMap::new();
        failed_config.insert("working_dir".to_string(), format!("{:?}", config.working_dir_strategy));
        failed_config.insert("separation".to_string(), format!("{:?}", config.instance_separation));
        failed_config.insert("layout".to_string(), config.layout.clone());

        self.config.failed_configs.push(FailedConfig {
            game_id,
            config: failed_config,
            failure_reason: error.to_string(),
            failed_at: SystemTime::now(),
        });

        // Limit the number of stored failures to prevent unbounded growth
        if self.config.failed_configs.len() > 1000 {
            self.config.failed_configs.drain(0..100); // Remove oldest 100 failures
        }

        self.save_config()?;
        Ok(())
    }

    /// Get recommended configuration based on learned patterns
    pub fn get_recommended_config(
        &self,
        game_id: &str,
        profile: &GameProfile,
    ) -> Option<RecommendedConfig> {
        // First, check if we have specific adaptation for this game
        if let Some(adaptation) = self.get_game_adaptation(game_id) {
            return Some(RecommendedConfig {
                confidence: self.calculate_confidence(adaptation),
                working_dir_strategy: adaptation.optimal_config.working_dir_strategy.clone(),
                separation_level: adaptation.optimal_config.separation_level.clone(),
                ports: adaptation.optimal_config.ports.clone(),
                layout: adaptation.optimal_config.layout.clone(),
                launch_args: adaptation.working_args.clone(),
                env_vars: adaptation.working_env_vars.clone(),
                notes: adaptation.notes.clone(),
            });
        }

        // Look for patterns based on game engine
        let engine_str = profile.engine.as_ref().map(|e| format!("{:?}", e));
        let mut best_pattern: Option<&SuccessPattern> = None;
        let mut best_score = 0.0;

        for pattern in &self.config.success_patterns {
            if pattern.engine == engine_str {
                let score = pattern.success_rate * (pattern.success_count as f64).ln();
                if score > best_score {
                    best_score = score;
                    best_pattern = Some(pattern);
                }
            }
        }

        if let Some(pattern) = best_pattern {
            Some(RecommendedConfig {
                confidence: (pattern.success_rate * 0.7).min(0.9), // Lower confidence for pattern-based
                working_dir_strategy: pattern.config.get("working_dir")
                    .cloned()
                    .unwrap_or_else(|| "SeparateDirectories".to_string()),
                separation_level: pattern.config.get("separation")
                    .cloned()
                    .unwrap_or_else(|| "Environment".to_string()),
                ports: vec![7777, 7778, 7779, 7780], // Default ports
                layout: pattern.config.get("layout")
                    .cloned()
                    .unwrap_or_else(|| "horizontal".to_string()),
                launch_args: Vec::new(),
                env_vars: HashMap::new(),
                notes: vec![format!("Based on pattern with {:.1}% success rate", pattern.success_rate * 100.0)],
            })
        } else {
            None
        }
    }

    /// Calculate confidence score for an adaptation
    fn calculate_confidence(&self, adaptation: &GameAdaptation) -> f64 {
        let base_confidence = (adaptation.success_count as f64 / (adaptation.success_count as f64 + 1.0)).min(0.95);
        
        // Reduce confidence if the last success was long ago
        let time_since_success = SystemTime::now()
            .duration_since(adaptation.last_success)
            .unwrap_or(Duration::from_secs(0));
        
        let time_factor = if time_since_success.as_secs() > 86400 * 30 { // 30 days
            0.8
        } else if time_since_success.as_secs() > 86400 * 7 { // 7 days
            0.9
        } else {
            1.0
        };

        base_confidence * time_factor
    }

    /// Check if a configuration is known to fail
    pub fn is_known_failure(&self, game_id: &str, config: &HashMap<String, String>) -> bool {
        self.config.failed_configs.iter().any(|failed| {
            failed.game_id == game_id && failed.config == *config
        })
    }

    /// Get statistics about the adaptive configuration
    pub fn get_stats(&self) -> AdaptiveStats {
        let total_games = self.config.game_adaptations.len();
        let total_successes: u32 = self.config.game_adaptations.values()
            .map(|a| a.success_count)
            .sum();
        let total_failures = self.config.failed_configs.len();

        let avg_success_rate = if !self.config.success_patterns.is_empty() {
            self.config.success_patterns.iter()
                .map(|p| p.success_rate)
                .sum::<f64>() / self.config.success_patterns.len() as f64
        } else {
            0.0
        };

        AdaptiveStats {
            total_games,
            total_successes,
            total_failures,
            avg_success_rate,
            patterns_learned: self.config.success_patterns.len(),
        }
    }
}

/// Recommended configuration based on learned patterns
#[derive(Debug, Clone)]
pub struct RecommendedConfig {
    pub confidence: f64,
    pub working_dir_strategy: String,
    pub separation_level: String,
    pub ports: Vec<u16>,
    pub layout: String,
    pub launch_args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub notes: Vec<String>,
}

/// Statistics about the adaptive configuration system
#[derive(Debug, Clone)]
pub struct AdaptiveStats {
    pub total_games: usize,
    pub total_successes: u32,
    pub total_failures: usize,
    pub avg_success_rate: f64,
    pub patterns_learned: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            game_adaptations: HashMap::new(),
            success_patterns: Vec::new(),
            failed_configs: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_adaptive_config_creation() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("adaptive.toml");
        
        let manager = AdaptiveConfigManager::new(config_path).unwrap();
        assert_eq!(manager.config.game_adaptations.len(), 0);
    }

    #[test]
    fn test_success_recording() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("adaptive.toml");
        
        let mut manager = AdaptiveConfigManager::new(config_path).unwrap();
        
        let profile = GameProfile {
            executable_pattern: "test.exe".to_string(),
            engine: Some(GameEngine::Unity),
            default_ports: vec![7777],
            default_layout: "horizontal".to_string(),
            multi_instance_support: crate::game_detection::MultiInstanceSupport::Native,
            launch_args: vec![],
            environment_vars: HashMap::new(),
            working_dir_strategy: crate::game_detection::WorkingDirStrategy::SeparateDirectories,
        };

        let config = crate::game_detection::GameConfiguration {
            ports: vec![7777],
            layout: "horizontal".to_string(),
            launch_args: vec![],
            environment_vars: HashMap::new(),
            working_dir_strategy: crate::game_detection::WorkingDirStrategy::SeparateDirectories,
            instance_separation: crate::game_detection::InstanceSeparation::Environment,
        };

        manager.record_success(
            "test_game".to_string(),
            &profile,
            &config,
            Duration::from_millis(1000),
        ).unwrap();

        assert_eq!(manager.config.game_adaptations.len(), 1);
        assert!(manager.get_game_adaptation("test_game").is_some());
    }
}