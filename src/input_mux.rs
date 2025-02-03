use evdev::{Device, InputEvent, InputEventKind};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::env;
use std::sync::{Arc, RwLock};
use log::info;
use std::fs;

pub struct InputMux {
    devices: HashMap<String, Device>,
    instance_map: HashMap<String, usize>,
    active_instances: Arc<RwLock<HashMap<usize, bool>>>,
}

impl InputMux {
    pub fn new() -> Self {
        InputMux {
            devices: HashMap::new(),
            instance_map: HashMap::new(),
            active_instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn enumerate_devices(&mut self) -> io::Result<()> {
        let input_path = env::var("INPUT_PATH").unwrap_or_else(|_| "/dev/input".to_string());
        let input_path = Path::new(&input_path);
        for entry in fs::read_dir(input_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Ok(device) = Device::open(&path) {
                    let name = device.name().unwrap_or_else(|| "Unknown".to_string());
                    info!("Found device: {}", name);
                    self.devices.insert(path.to_string_lossy().to_string(), device);
                }
            }
        }
        Ok(())
    }

    pub fn capture_events(&self) {
        for (path, device) in &self.devices {
            let device = device.clone();
            let instance = self.instance_map.get(path).cloned().unwrap_or(0);
            let active_instances = Arc::clone(&self.active_instances);

            std::thread::spawn(move || {
                loop {
                    match device.next_event(evdev::ReadFlag::NORMAL) {
                        Ok(event) => {
                            if let Some(event) = event {
                                info!("Captured event from device {}: {:?}", path, event);
                                self.handle_event(event, instance, &active_instances);
                            }
                        }
                        Err(e) => eprintln!("Error reading event: {}", e),
                    }
                }
            });
        }
    }

    fn handle_event(&self, event: InputEvent, instance: usize, active_instances: &Arc<RwLock<HashMap<usize, bool>>>) {
        let mut active_instances = active_instances.write().unwrap();
        if let Some(&active) = active_instances.get(&instance) {
            if !active {
                active_instances.insert(instance, true);
                // Forward event to the game instance
                info!("Forwarding event to instance {}: {:?}", instance, event);
                // Reset the active state after handling the event
                active_instances.insert(instance, false);
            }
        }
    }

    pub fn map_device_to_instance(&mut self, device_path: &str, instance: usize) {
        self.instance_map.insert(device_path.to_string(), instance);
    }
}

fn main() {
    let mut input_mux = InputMux::new();

    // Enumerate connected input devices
    if let Err(e) = input_mux.enumerate_devices() {
        eprintln!("Error enumerating devices: {}", e);
        return;
    }

    // Map devices to game instances
    input_mux.map_device_to_instance("/dev/input/event0", 1);
    input_mux.map_device_to_instance("/dev/input/event1", 2);

    // Capture raw input events from each device
    input_mux.capture_events();

    // Keep the main thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
