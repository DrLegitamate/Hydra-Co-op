//! Game Compatibility Checker
//! 
//! Analyzes games for potential compatibility issues with multi-instance launching

use std::path::Path;
use std::fs;
use log::{info, warn};
use crate::errors::{HydraError, Result};

#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    pub game_path: String,
    pub compatibility_score: u8, // 0-100
    pub issues: Vec<CompatibilityIssue>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CompatibilityIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub workaround: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IssueSeverity {
    Info,
    Warning,
    Critical,
}

pub struct CompatibilityChecker;

impl CompatibilityChecker {
    pub fn analyze_game(game_path: &Path) -> Result<CompatibilityReport> {
        info!("Analyzing game compatibility: {}", game_path.display());
        
        let mut report = CompatibilityReport {
            game_path: game_path.to_string_lossy().to_string(),
            compatibility_score: 100,
            issues: Vec::new(),
            recommendations: Vec::new(),
        };

        // Check for known problematic files
        Self::check_anti_cheat(&mut report, game_path);
        Self::check_drm_systems(&mut report, game_path);
        Self::check_launcher_dependencies(&mut report, game_path);
        Self::check_network_requirements(&mut report, game_path);

        // Calculate final compatibility score
        report.compatibility_score = Self::calculate_score(&report.issues);

        Ok(report)
    }

    fn check_anti_cheat(report: &mut CompatibilityReport, game_path: &Path) {
        let game_dir = game_path.parent().unwrap_or(Path::new("."));
        
        let anti_cheat_files = [
            "EasyAntiCheat.exe",
            "BEService.exe", // BattlEye
            "VAC.dll",
            "steam_api.dll", // May indicate Steam DRM
        ];

        for file in &anti_cheat_files {
            if game_dir.join(file).exists() {
                let issue = CompatibilityIssue {
                    severity: IssueSeverity::Critical,
                    description: format!("Anti-cheat system detected: {}", file),
                    workaround: Some("Consider using different user accounts or sandboxing".to_string()),
                };
                report.issues.push(issue);
            }
        }
    }

    fn check_drm_systems(report: &mut CompatibilityReport, game_path: &Path) {
        let game_dir = game_path.parent().unwrap_or(Path::new("."));
        
        let drm_indicators = [
            "steam_api64.dll",
            "denuvo.dll",
            "activation.dll",
        ];

        for file in &drm_indicators {
            if game_dir.join(file).exists() {
                let issue = CompatibilityIssue {
                    severity: IssueSeverity::Warning,
                    description: format!("DRM system detected: {}", file),
                    workaround: Some("May require separate game installations per instance".to_string()),
                };
                report.issues.push(issue);
            }
        }
    }

    fn check_launcher_dependencies(report: &mut CompatibilityReport, game_path: &Path) {
        let game_dir = game_path.parent().unwrap_or(Path::new("."));
        
        // Check for launcher executables that might interfere
        let launcher_files = [
            "launcher.exe",
            "updater.exe",
            "patcher.exe",
        ];

        for file in &launcher_files {
            if game_dir.join(file).exists() {
                let issue = CompatibilityIssue {
                    severity: IssueSeverity::Info,
                    description: format!("Game launcher detected: {}", file),
                    workaround: Some("Launch game executable directly, not through launcher".to_string()),
                };
                report.issues.push(issue);
                report.recommendations.push("Use direct game executable instead of launcher".to_string());
            }
        }
    }

    fn check_network_requirements(report: &mut CompatibilityReport, game_path: &Path) {
        // This is a simplified check - in practice, you'd analyze the executable
        // or configuration files for network-related settings
        
        let game_name = game_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if game_name.contains("online") || game_name.contains("multiplayer") {
            report.recommendations.push("Ensure network ports are properly configured".to_string());
        }
    }

    fn calculate_score(issues: &[CompatibilityIssue]) -> u8 {
        let mut score = 100u8;
        
        for issue in issues {
            let penalty = match issue.severity {
                IssueSeverity::Info => 5,
                IssueSeverity::Warning => 15,
                IssueSeverity::Critical => 40,
            };
            score = score.saturating_sub(penalty);
        }
        
        score
    }

    pub fn print_report(report: &CompatibilityReport) {
        println!("=== Compatibility Report ===");
        println!("Game: {}", report.game_path);
        println!("Compatibility Score: {}/100", report.compatibility_score);
        
        if !report.issues.is_empty() {
            println!("\nIssues Found:");
            for issue in &report.issues {
                let severity_str = match issue.severity {
                    IssueSeverity::Info => "INFO",
                    IssueSeverity::Warning => "WARN",
                    IssueSeverity::Critical => "CRIT",
                };
                println!("  [{}] {}", severity_str, issue.description);
                if let Some(workaround) = &issue.workaround {
                    println!("    Workaround: {}", workaround);
                }
            }
        }
        
        if !report.recommendations.is_empty() {
            println!("\nRecommendations:");
            for rec in &report.recommendations {
                println!("  • {}", rec);
            }
        }
    }
}