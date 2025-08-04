//! Audio Management for Multi-Instance Gaming
//! 
//! Provides per-instance audio routing using PulseAudio/PipeWire

use std::process::Command;
use std::collections::HashMap;
use log::{info, warn, error};
use crate::errors::{HydraError, Result};

pub struct AudioManager {
    virtual_sinks: HashMap<usize, String>,
    audio_system: AudioSystem,
}

#[derive(Debug)]
enum AudioSystem {
    PulseAudio,
    PipeWire,
    ALSA,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        let audio_system = Self::detect_audio_system()?;
        info!("Detected audio system: {:?}", audio_system);
        
        Ok(Self {
            virtual_sinks: HashMap::new(),
            audio_system,
        })
    }

    fn detect_audio_system() -> Result<AudioSystem> {
        // Check for PipeWire
        if Command::new("pw-cli").arg("info").output().is_ok() {
            return Ok(AudioSystem::PipeWire);
        }
        
        // Check for PulseAudio
        if Command::new("pactl").arg("info").output().is_ok() {
            return Ok(AudioSystem::PulseAudio);
        }
        
        // Fallback to ALSA
        Ok(AudioSystem::ALSA)
    }

    pub fn create_virtual_sinks(&mut self, num_instances: usize) -> Result<()> {
        match self.audio_system {
            AudioSystem::PulseAudio => self.create_pulse_sinks(num_instances),
            AudioSystem::PipeWire => self.create_pipewire_sinks(num_instances),
            AudioSystem::ALSA => {
                warn!("ALSA detected - virtual audio sinks not supported");
                Ok(())
            }
        }
    }

    fn create_pulse_sinks(&mut self, num_instances: usize) -> Result<()> {
        for i in 0..num_instances {
            let sink_name = format!("hydra_game_{}", i);
            let sink_description = format!("Hydra Co-op Game Instance {}", i);
            
            let output = Command::new("pactl")
                .args(&[
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={}", sink_name),
                    &format!("sink_properties=device.description=\"{}\"", sink_description),
                ])
                .output()
                .map_err(HydraError::Io)?;

            if output.status.success() {
                self.virtual_sinks.insert(i, sink_name.clone());
                info!("Created PulseAudio virtual sink: {}", sink_name);
            } else {
                error!("Failed to create PulseAudio sink: {}", 
                       String::from_utf8_lossy(&output.stderr));
            }
        }
        Ok(())
    }

    fn create_pipewire_sinks(&mut self, num_instances: usize) -> Result<()> {
        for i in 0..num_instances {
            let sink_name = format!("hydra_game_{}", i);
            
            // PipeWire virtual sink creation (simplified)
            let output = Command::new("pw-cli")
                .args(&[
                    "create-node",
                    "adapter",
                    &format!("{{\"factory.name\":\"support.null-audio-sink\",\"node.name\":\"{}\"}}", sink_name),
                ])
                .output()
                .map_err(HydraError::Io)?;

            if output.status.success() {
                self.virtual_sinks.insert(i, sink_name.clone());
                info!("Created PipeWire virtual sink: {}", sink_name);
            }
        }
        Ok(())
    }

    pub fn route_game_audio(&self, instance_id: usize, game_pid: u32) -> Result<()> {
        if let Some(sink_name) = self.virtual_sinks.get(&instance_id) {
            match self.audio_system {
                AudioSystem::PulseAudio => {
                    // Move all streams from this PID to the virtual sink
                    let output = Command::new("bash")
                        .args(&[
                            "-c",
                            &format!(
                                "pactl list short sink-inputs | grep {} | cut -f1 | xargs -I{{}} pactl move-sink-input {{}} {}",
                                game_pid, sink_name
                            ),
                        ])
                        .output()
                        .map_err(HydraError::Io)?;

                    if output.status.success() {
                        info!("Routed audio for PID {} to sink {}", game_pid, sink_name);
                    }
                }
                AudioSystem::PipeWire => {
                    // PipeWire audio routing (more complex, would need pw-link)
                    info!("PipeWire audio routing for PID {} (implementation needed)", game_pid);
                }
                AudioSystem::ALSA => {
                    warn!("ALSA audio routing not implemented");
                }
            }
        }
        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        match self.audio_system {
            AudioSystem::PulseAudio => {
                for sink_name in self.virtual_sinks.values() {
                    let _ = Command::new("pactl")
                        .args(&["unload-module", "module-null-sink"])
                        .output();
                }
            }
            AudioSystem::PipeWire => {
                for sink_name in self.virtual_sinks.values() {
                    let _ = Command::new("pw-cli")
                        .args(&["destroy", sink_name])
                        .output();
                }
            }
            AudioSystem::ALSA => {}
        }
        Ok(())
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            virtual_sinks: HashMap::new(),
            audio_system: AudioSystem::ALSA,
        })
    }
}