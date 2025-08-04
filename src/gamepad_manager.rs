//! Enhanced Gamepad Management
//! 
//! Provides specialized handling for gamepad devices with Steam Input integration

use std::collections::HashMap;
use std::path::Path;
use evdev::{Device, InputEventKind, Key};
use log::{info, warn, debug};
use crate::errors::{HydraError, Result};
use crate::input_mux::{DeviceIdentifier, InputMux};

/// Specialized gamepad manager for enhanced controller support
pub struct GamepadManager {
    gamepads: HashMap<DeviceIdentifier, GamepadInfo>,
    steam_input_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct GamepadInfo {
    pub device_id: DeviceIdentifier,
    pub controller_type: ControllerType,
    pub capabilities: GamepadCapabilities,
    pub steam_config_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ControllerType {
    Xbox360,
    XboxOne,
    PS4,
    PS5,
    SteamController,
    Generic,
}

#[derive(Debug, Clone)]
pub struct GamepadCapabilities {
    pub has_analog_sticks: bool,
    pub has_triggers: bool,
    pub has_dpad: bool,
    pub button_count: u8,
}

impl GamepadManager {
    pub fn new() -> Self {
        Self {
            gamepads: HashMap::new(),
            steam_input_enabled: Self::detect_steam_input(),
        }
    }

    /// Detect if Steam Input is available
    fn detect_steam_input() -> bool {
        // Check for Steam runtime or Steam Input libraries
        std::env::var("STEAM_RUNTIME").is_ok() || 
        Path::new("/usr/lib/steam/steamapps/common").exists()
    }

    /// Scan for and classify gamepad devices
    pub fn scan_gamepads(&mut self, input_mux: &InputMux) -> Result<()> {
        info!("Scanning for gamepad devices...");
        
        let devices = input_mux.get_available_devices();
        
        for device_id in devices {
            if self.is_gamepad_device(&device_id) {
                let gamepad_info = self.analyze_gamepad(&device_id)?;
                info!("Detected gamepad: {} ({})", device_id.name, format!("{:?}", gamepad_info.controller_type));
                self.gamepads.insert(device_id, gamepad_info);
            }
        }

        info!("Found {} gamepad devices", self.gamepads.len());
        Ok(())
    }

    /// Check if a device is likely a gamepad
    fn is_gamepad_device(&self, device_id: &DeviceIdentifier) -> bool {
        let name_lower = device_id.name.to_lowercase();
        
        // Common gamepad identifiers
        let gamepad_keywords = [
            "gamepad", "controller", "joystick", "xbox", "playstation", 
            "ps4", "ps5", "steam", "8bitdo", "logitech"
        ];

        gamepad_keywords.iter().any(|keyword| name_lower.contains(keyword)) ||
        // Check vendor IDs for known gamepad manufacturers
        self.is_gamepad_vendor_id(device_id.vendor_id)
    }

    fn is_gamepad_vendor_id(&self, vendor_id: u16) -> bool {
        match vendor_id {
            0x045e => true, // Microsoft
            0x054c => true, // Sony
            0x28de => true, // Valve Steam Controller
            0x046d => true, // Logitech
            0x0e6f => true, // Logic3
            0x0f0d => true, // Hori
            _ => false,
        }
    }

    /// Analyze gamepad capabilities and type
    fn analyze_gamepad(&self, device_id: &DeviceIdentifier) -> Result<GamepadInfo> {
        let controller_type = self.detect_controller_type(device_id);
        let capabilities = self.detect_capabilities(device_id);
        
        let steam_config_path = if self.steam_input_enabled {
            self.find_steam_config(device_id)
        } else {
            None
        };

        Ok(GamepadInfo {
            device_id: device_id.clone(),
            controller_type,
            capabilities,
            steam_config_path,
        })
    }

    fn detect_controller_type(&self, device_id: &DeviceIdentifier) -> ControllerType {
        let name_lower = device_id.name.to_lowercase();
        
        if name_lower.contains("xbox 360") || device_id.product_id == 0x028e {
            ControllerType::Xbox360
        } else if name_lower.contains("xbox") || name_lower.contains("microsoft") {
            ControllerType::XboxOne
        } else if name_lower.contains("ps4") || name_lower.contains("dualshock 4") {
            ControllerType::PS4
        } else if name_lower.contains("ps5") || name_lower.contains("dualsense") {
            ControllerType::PS5
        } else if name_lower.contains("steam") && device_id.vendor_id == 0x28de {
            ControllerType::SteamController
        } else {
            ControllerType::Generic
        }
    }

    fn detect_capabilities(&self, device_id: &DeviceIdentifier) -> GamepadCapabilities {
        // This would require opening the actual evdev device to check capabilities
        // For now, provide reasonable defaults based on controller type
        GamepadCapabilities {
            has_analog_sticks: true,
            has_triggers: true,
            has_dpad: true,
            button_count: 14, // Standard gamepad button count
        }
    }

    fn find_steam_config(&self, device_id: &DeviceIdentifier) -> Option<String> {
        // Look for Steam Input configurations
        let steam_config_dirs = [
            "~/.steam/steam/config",
            "~/.local/share/Steam/config",
            "/usr/share/steam/config",
        ];

        for config_dir in &steam_config_dirs {
            let expanded_dir = shellexpand::tilde(config_dir);
            let config_path = Path::new(expanded_dir.as_ref())
                .join("controller_configs")
                .join(format!("{:04x}_{:04x}.vdf", device_id.vendor_id, device_id.product_id));
            
            if config_path.exists() {
                return Some(config_path.to_string_lossy().to_string());
            }
        }

        None
    }

    /// Get gamepad-optimized input assignments
    pub fn get_gamepad_assignments(&self, num_instances: usize) -> Vec<DeviceIdentifier> {
        let mut assignments = Vec::new();
        let mut gamepad_iter = self.gamepads.keys();

        for _ in 0..num_instances {
            if let Some(gamepad_id) = gamepad_iter.next() {
                assignments.push(gamepad_id.clone());
            }
        }

        assignments
    }

    /// Apply gamepad-specific optimizations
    pub fn optimize_for_game(&self, game_name: &str) -> Result<HashMap<String, String>> {
        let mut optimizations = HashMap::new();

        // Game-specific gamepad optimizations
        match game_name.to_lowercase().as_str() {
            name if name.contains("borderlands") => {
                optimizations.insert("GAMEPAD_DEADZONE".to_string(), "0.15".to_string());
                optimizations.insert("GAMEPAD_SENSITIVITY".to_string(), "1.2".to_string());
            },
            name if name.contains("call of duty") => {
                optimizations.insert("GAMEPAD_AIM_ASSIST".to_string(), "1".to_string());
                optimizations.insert("GAMEPAD_VIBRATION".to_string(), "1".to_string());
            },
            _ => {
                // Default optimizations
                optimizations.insert("GAMEPAD_ENABLED".to_string(), "1".to_string());
            }
        }

        Ok(optimizations)
    }
}

impl Default for GamepadManager {
    fn default() -> Self {
        Self::new()
    }
}