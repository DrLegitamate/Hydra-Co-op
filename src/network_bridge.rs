//! Enhanced Network Bridge for Complex Game Networking
//! 
//! Provides TAP/TUN interface support for games requiring more sophisticated
//! network topologies beyond simple UDP relay.

use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;
use log::{info, warn, error};
use crate::errors::{HydraError, Result};

/// Network bridge for creating virtual network interfaces
pub struct NetworkBridge {
    bridge_name: String,
    tap_interfaces: Vec<String>,
    ip_range: Ipv4Addr,
}

impl NetworkBridge {
    pub fn new(bridge_name: String) -> Self {
        Self {
            bridge_name,
            tap_interfaces: Vec::new(),
            ip_range: Ipv4Addr::new(192, 168, 100, 1),
        }
    }

    /// Create a virtual bridge interface for complex networking scenarios
    pub fn create_bridge(&mut self, num_instances: usize) -> Result<()> {
        info!("Creating network bridge '{}' for {} instances", self.bridge_name, num_instances);

        // Create bridge interface
        let output = Command::new("ip")
            .args(&["link", "add", "name", &self.bridge_name, "type", "bridge"])
            .output()
            .map_err(HydraError::Io)?;

        if !output.status.success() {
            return Err(HydraError::application(format!(
                "Failed to create bridge: {}", 
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Bring bridge up
        Command::new("ip")
            .args(&["link", "set", "dev", &self.bridge_name, "up"])
            .output()
            .map_err(HydraError::Io)?;

        // Assign IP to bridge
        let bridge_ip = format!("{}/24", self.ip_range);
        Command::new("ip")
            .args(&["addr", "add", &bridge_ip, "dev", &self.bridge_name])
            .output()
            .map_err(HydraError::Io)?;

        // Create TAP interfaces for each instance
        for i in 0..num_instances {
            let tap_name = format!("hydra_tap_{}", i);
            self.create_tap_interface(&tap_name, i)?;
            self.tap_interfaces.push(tap_name);
        }

        info!("Network bridge setup complete");
        Ok(())
    }

    fn create_tap_interface(&self, tap_name: &str, instance_id: usize) -> Result<()> {
        // Create TAP interface
        Command::new("ip")
            .args(&["tuntap", "add", "dev", tap_name, "mode", "tap"])
            .output()
            .map_err(HydraError::Io)?;

        // Add to bridge
        Command::new("ip")
            .args(&["link", "set", "dev", tap_name, "master", &self.bridge_name])
            .output()
            .map_err(HydraError::Io)?;

        // Bring interface up
        Command::new("ip")
            .args(&["link", "set", "dev", tap_name, "up"])
            .output()
            .map_err(HydraError::Io)?;

        // Assign IP to TAP interface
        let tap_ip = format!("192.168.100.{}/24", instance_id + 2);
        Command::new("ip")
            .args(&["addr", "add", &tap_ip, "dev", tap_name])
            .output()
            .map_err(HydraError::Io)?;

        info!("Created TAP interface {} with IP {}", tap_name, tap_ip);
        Ok(())
    }

    /// Clean up network interfaces
    pub fn cleanup(&self) -> Result<()> {
        info!("Cleaning up network bridge and TAP interfaces");

        // Remove TAP interfaces
        for tap_name in &self.tap_interfaces {
            let _ = Command::new("ip")
                .args(&["link", "delete", tap_name])
                .output();
        }

        // Remove bridge
        let _ = Command::new("ip")
            .args(&["link", "delete", &self.bridge_name])
            .output();

        Ok(())
    }
}