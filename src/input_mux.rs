use evdev::{Device, InputEvent, InputEventKind, ReadFlag};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write}; // Import Read and Write
use std::path::Path;
use std::env;
use std::sync::{Arc, RwLock};
use log::{info, warn, error, debug}; // Import debug log level
use std::thread::{self, JoinHandle}; // Import JoinHandle
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering}; // Import AtomicBool and Ordering

// Import serde for serialization support
use serde::{Deserialize, Serialize};
// We will use the uinput-rs crate for creating virtual input devices.
// Add this to your Cargo.toml:
// [dependencies]
// uinput = "0.5" # Or the latest version
// evdev = "0.12" # Ensure evdev version is >= 0.12 for read_with_timeout
// log = "0.4"
// env_logger = "0.11" # Or another logger

// Custom error type for input multiplexing operations
#[derive(Debug)]
pub enum InputMuxError {
    IoError(io::Error),
    EvdevError(evdev::Error),
    UinputError(uinput::Error),
    DeviceNotFound(String),
    MissingDeviceInfo, // Consider removing or making more specific if not used
    GenericError(String),
    AlreadyRunning, // Added error for starting capture when already running
}

impl std::fmt::Display for InputMuxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InputMuxError::IoError(e) => write!(f, "I/O error: {}", e),
            InputMuxError::EvdevError(e) => write!(f, "evdev error: {}", e),
            InputMuxError::UinputError(e) => write!(f, "uinput error: {}", e),
            InputMuxError::DeviceNotFound(name) => write!(f, "Input device not found: {}", name),
            InputMuxError::MissingDeviceInfo => write!(f, "Missing device information"), // Check if still needed
            InputMuxError::GenericError(msg) => write!(f, "Input multiplexer error: {}", msg),
            InputMuxError::AlreadyRunning => write!(f, "Input capture is already running"),
        }
    }
}

impl std::error::Error for InputMuxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InputMuxError::IoError(e) => Some(e),
            InputMuxError::EvdevError(e) => Some(e),
            InputMuxError::UinputError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for InputMuxError {
    fn from(err: io::Error) -> Self {
        InputMuxError::IoError(err)
    }
}

impl From<evdev::Error> for InputMuxError {
    fn from(err: evdev::Error) -> Self {
        InputMuxError::EvdevError(err)
    }
}

impl From<uinput::Error> for InputMuxError {
    fn from(err: uinput::Error) -> Self {
        InputMuxError::UinputError(err)
    }
}


/// Represents information needed to identify and map an input device.
/// Using name, physical location, and ID for more robust identification than just path.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentifier { // Made pub
    pub name: String, // Made pub
    pub phys: Option<String>, // Made pub
    pub bustype: u16, // Made pub
    pub vendor_id: u16, // Made pub
    pub product_id: u16, // Made pub
    pub version: u16, // Made pub
}

/// Represents different ways to assign input devices to game instances
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputAssignment {
    /// Automatically detect and assign the next available device
    AutoDetect,
    /// Assign a specific device by its identifier
    Device(DeviceIdentifier),
    /// No device assigned to this instance
    None,
}
impl From<&Device> for DeviceIdentifier {
    fn from(device: &Device) -> Self {
        let input_id = device.input_id();
        DeviceIdentifier {
            name: device.name().unwrap_or("Unknown").to_string(),
            phys: device.phys().map(|s| s.to_string()),
            bustype: input_id.bustype(),
            vendor_id: input_id.vendor(),
            product_id: input_id.product(),
            version: input_id.version(),
        }
    }
}


pub struct InputMux {
    // Map DeviceIdentifier to the opened evdev::Device
    devices: HashMap<DeviceIdentifier, Device>,
    // Map DeviceIdentifier to the instance index (0, 1, 2...)
    instance_map: HashMap<DeviceIdentifier, usize>,
    // Map instance index to its virtual uinput device
    virtual_devices: HashMap<usize, uinput::Device>,
    // Flag to signal capture threads to stop
    running: Arc<AtomicBool>,
    // Store join handles for capture threads to wait on
    capture_threads: Option<Vec<JoinHandle<()>>>, // Use Option to manage running state

}

impl InputMux {
    pub fn new() -> Self {
        info!("Creating new InputMux instance.");
        InputMux {
            devices: HashMap::new(),
            instance_map: HashMap::new(),
            virtual_devices: HashMap::new(),
            running: Arc::new(AtomicBool::new(false)), // Initially not running
            capture_threads: None,
        }
    }

    /// Enumerates connected input devices in /dev/input.
    /// Requires read permissions on /dev/input/event* files.
    pub fn enumerate_devices(&mut self) -> Result<(), InputMuxError> {
        info!("Enumerating input devices in /dev/input...");
        let input_path = env::var("INPUT_PATH").unwrap_or_else(|_| "/dev/input".to_string());
        let input_dir = Path::new(&input_path);

        if !input_dir.exists() {
            warn!("Input directory '{}' does not exist.", input_path);
            return Err(InputMuxError::IoError(io::Error::new(io::ErrorKind::NotFound, format!("Input directory '{}' not found", input_path))));
        }

        if !input_dir.is_dir() {
            warn!("Input path '{}' is not a directory.", input_path);
            return Err(InputMuxError::IoError(io::Error::new(io::ErrorKind::Other, format!("Input path '{}' is not a directory", input_path))));
        }

        // Clear previously enumerated devices before re-enumerating
        self.devices.clear();

        // Use ? for fs::read_dir error propagation
        for entry in fs::read_dir(input_dir)? {
            // Use ? for entry result error propagation
            let entry = entry?;
            let path = entry.path();
            debug!("Found potential device path: {}", path.display());

            // Only consider event files
            if path.is_file() && path.file_name().and_then(|name| name.to_str()).unwrap_or("").starts_with("event") {
                debug!("Opening device: {}", path.display());
                match Device::open(&path) {
                    Ok(device) => {
                        let identifier = DeviceIdentifier::from(&device);
                        info!("Found device: {}", identifier.name);
                        debug!("Device details: {:?}", identifier);
                        self.devices.insert(identifier, device);
                    }
                    Err(e) => {
                        // Log the error and continue to the next device
                        warn!("Failed to open device {}: {}", path.display(), e);
                    }
                }
            } else {
                debug!("Skipping non-event file or directory: {}", path.display());
            }
        }

        if self.devices.is_empty() {
            warn!("No input devices found in {}. Ensure you have read permissions on /dev/input/event* files.", input_path);
        } else {
            info!("Finished enumerating devices. Found {} usable devices.", self.devices.len());
        }

        Ok(())
    }

    /// Creates virtual uinput devices for each game instance.
    /// Game instances will listen to these virtual devices.
    /// Requires write permissions on /dev/uinput.
    pub fn create_virtual_devices(&mut self, num_instances: usize) -> Result<(), InputMuxError> {
        info!("Creating virtual input devices for {} instances...", num_instances);
        // Clear previously created virtual devices
        self.virtual_devices.clear();

        // TODO: Configure virtual device capabilities based on collected physical device capabilities.
        // For a real application, you'd iterate through `self.devices` to collect
        // all supported event types (keys, relative, absolute, etc.) and their codes,
        // then register them with the uinput builder.
        // Example (simplified):
        // let mut builder = uinput::Builder::new()?;
        // for (_, device) in &self.devices {
        //     if let Ok(keys) = device.supported_keys() {
        //         for key in keys.iter() {
        //             builder = builder.event(uinput::event::Key::new(key))?;
        //         }
        //     }
        //     if let Ok(rel_axes) = device.supported_relative_axes() {
        //          for axis in rel_axes.iter() {
        //              builder = builder.event(uinput::event::Relative::new(axis))?;
        //          }
        //     }
        //     // ... add other event types
        // }
        // Then use this configured builder for each virtual device.

        for i in 0..num_instances {
            // Create a unique name for each virtual device instance
            let device_name = format!("HydraCoop Virtual Device {}", i);
            debug!("Creating virtual device: {}", device_name);

            // For now, create a basic virtual device with some common capabilities
            let virtual_device = uinput::Builder::new()?
                .name(&device_name)?
                .event(uinput::event::Relative::Relative)? // Example: Enable relative motion events (mouse)
                .event(uinput::event::Key::Enter)? // Example: Enable Enter key
                .event(uinput::event::Key::Space)? // Example: Enable Space key
                 // Add more capabilities as needed for the games/input types you support
                .create()?;

            info!("Created virtual device for instance {}: {}", i, virtual_device.sysname()); // Use sysname to get the /dev/input/eventX name
            self.virtual_devices.insert(i, virtual_device);
        }

        info!("Finished creating virtual devices. Created {} devices.", self.virtual_devices.len());
        Ok(())
    }


    /// Maps a physical input device to a specific game instance.
    /// The device is identified by its name. This function is less robust
    /// than using `map_device_to_instance_by_identifier` if multiple devices
    /// share the same name.
    pub fn map_device_to_instance(&mut self, device_name: &str, instance_index: usize) -> Result<(), InputMuxError> {
        info!("Attempting to map device '{}' to instance index {} by name.", device_name, instance_index);

        // Find the DeviceIdentifier for the given device name
        let device_identifier = self.devices.keys()
            .find(|id| id.name == device_name)
            .cloned(); // Clone the identifier to use

        match device_identifier {
            Some(identifier) => {
                // Delegate to the more robust identifier-based mapping
                self.map_device_to_instance_by_identifier(identifier, instance_index)
            }
            None => {
                warn!("Physical input device '{}' not found by name. Cannot map to instance {}. Available devices: {:?}", device_name, instance_index, self.devices.keys());
                Err(InputMuxError::DeviceNotFound(device_name.to_string()))
            }
        }
    }


    /// Captures events from mapped physical devices and injects them into the
    /// corresponding virtual devices for each instance.
    /// This function spawns a thread for each mapped physical device.
    pub fn capture_events(&mut self, assignments: &[(usize, InputAssignment)]) -> Result<(), InputMuxError> {
        // Clear existing mappings
        self.instance_map.clear();
        
        // Process input assignments
        let mut auto_detect_queue: Vec<DeviceIdentifier> = self.devices.keys().cloned().collect();
        let mut used_devices: std::collections::HashSet<DeviceIdentifier> = std::collections::HashSet::new();
        
        for &(instance_index, ref assignment) in assignments {
            match assignment {
                InputAssignment::Device(device_id) => {
                    if self.devices.contains_key(device_id) && !used_devices.contains(device_id) {
                        self.instance_map.insert(device_id.clone(), instance_index);
                        used_devices.insert(device_id.clone());
                        info!("Assigned device '{}' to instance {}", device_id.name, instance_index);
                    } else {
                        warn!("Device '{}' not available for instance {}", device_id.name, instance_index);
                    }
                }
                InputAssignment::AutoDetect => {
                    if let Some(device_id) = auto_detect_queue.iter()
                        .find(|id| !used_devices.contains(id))
                        .cloned() 
                    {
                        self.instance_map.insert(device_id.clone(), instance_index);
                        used_devices.insert(device_id.clone());
                        info!("Auto-assigned device '{}' to instance {}", device_id.name, instance_index);
                    } else {
                        warn!("No available device for auto-detection for instance {}", instance_index);
                    }
                }
                InputAssignment::None => {
                    info!("No input device assigned to instance {}", instance_index);
                }
            }
        }
        
        if self.running.load(Ordering::SeqCst) {
            warn!("Input capture is already running.");
            return Err(InputMuxError::AlreadyRunning);
        }

        if self.devices.is_empty() {
            warn!("No input devices enumerated. Skipping event capture.");
            return Ok(()); // Or return an error if no devices is considered a fatal issue
        }

        if self.virtual_devices.is_empty() {
            error!("No virtual devices created. Cannot route input events.");
            return Err(InputMuxError::GenericError("No virtual devices available for routing".to_string()));
        }

        if self.instance_map.is_empty() {
            warn!("No devices mapped to instances. Skipping event capture.");
            return Ok(()); // No mapping, nothing to capture/route
        }

        info!("Starting input event capture and routing...");
        self.running.store(true, Ordering::SeqCst); // Set running flag

        let mut join_handles = Vec::new();

        // Iterate over devices that are actually mapped to an instance
        for (identifier, instance_index) in &self.instance_map {
             // Find the actual device from the devices map
             if let Some(device) = self.devices.get(identifier) {
                let mut device = device.clone(); // Clone the device for the thread
                let identifier = identifier.clone(); // Clone the identifier
                let virtual_devices = self.virtual_devices.clone(); // Clone the map of virtual devices
                let running_flag = self.running.clone(); // Clone the running flag for the thread
                let instance_index = *instance_index; // Copy the instance index

                info!("Starting capture thread for device: {} (mapped to instance {})", identifier.name, instance_index);

                let handle = thread::spawn(move || {
                    // Get the virtual device for the target instance within the thread
                    let virtual_device = match virtual_devices.get(&instance_index) {
                        Some(dev) => dev,
                        None => {
                             error!("Capture thread: Virtual device for instance {} not found. Exiting thread for device '{}'.", instance_index, identifier.name);
                             return; // Exit thread if virtual device is missing
                        }
                    };

                    // Use a timeout to allow the thread to check the running flag periodically
                    let read_timeout = Duration::from_millis(100); // Check every 100ms

                    while running_flag.load(Ordering::SeqCst) {
                        match device.read_with_timeout(read_timeout) {
                            Ok(Some(event)) => {
                                debug!("Captured event from device '{}': {:?}", identifier.name, event);

                                // Inject the event into the virtual device
                                debug!("Injecting event to virtual device for instance {}: {:?}", instance_index, event);
                                if let Err(e) = virtual_device.write_event(&event) {
                                    error!("Failed to inject event for device '{}' to instance {}: {}", identifier.name, instance_index, e);
                                    // Depending on the error, you might want to break the loop or handle it differently
                                     // For critical errors, break; otherwise, log and continue.
                                     if e.kind() == io::ErrorKind::BrokenPipe {
                                         error!("Broken pipe when writing to virtual device for instance {}. Exiting thread for device '{}'.", instance_index, identifier.name);
                                         break; // Stop thread on broken pipe
                                     }
                                } else {
                                    // Sync the virtual device after injecting events (especially button/key events)
                                    if event.kind() == InputEventKind::Key || event.kind() == InputEventKind::Button {
                                        if let Err(e) = virtual_device.synchronize() {
                                            error!("Failed to synchronize virtual device for instance {}: {}", instance_index, e);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                // Timeout occurred, continue the loop to check running_flag
                                debug!("Read timeout for device '{}', checking stop flag.", identifier.name);
                            }
                            Err(e) => {
                                // Handle errors reading from the device
                                error!("Error reading event from device '{}' ({:?}): {}", identifier.name, identifier, e);
                                match e.kind() {
                                    io::ErrorKind::BrokenPipe | io::ErrorKind::NotFound => {
                                        warn!("Device '{}' appears disconnected. Stopping capture for this device.", identifier.name);
                                        break; // Stop the thread for this device
                                    }
                                     io::ErrorKind::Interrupted => {
                                         // Read was interrupted by a signal, retry
                                         debug!("Read interrupted for device '{}', retrying.", identifier.name);
                                         continue;
                                     }
                                     // Handle other IO errors as needed
                                    _ => {
                                         error!("Unhandled IO error for device '{}'. Exiting thread.", identifier.name);
                                         break;
                                     }
                                }
                            }
                        }
                    }
                    info!("Capture thread for device '{}' exited.", identifier.name);
                });
                join_handles.push(handle);
             } else {
                 error!("Mapped device identifier {:?} not found in enumerated devices. Cannot start capture thread for this mapping.", identifier);
             }
        }

        self.capture_threads = Some(join_handles);

        info!("Input event capture threads started.");
        Ok(())
    }

    /// Signals the capture threads to stop and waits for them to finish.
    pub fn stop_capture(&mut self) -> Result<(), InputMuxError> {
        if !self.running.load(Ordering::SeqCst) {
            info!("Input capture is not running.");
            return Ok(());
        }

        info!("Stopping input event capture...");
        self.running.store(false, Ordering::SeqCst); // Signal threads to stop

        // Wait for the threads to finish
        if let Some(handles) = self.capture_threads.take() {
            for handle in handles {
                if let Err(e) = handle.join() {
                    error!("Failed to join capture thread: {:?}", e);
                }
            }
            info!("All capture threads joined.");
        } else {
             warn!("No capture threads found to join.");
        }
        Ok(())
    }


    /// Maps a physical input device identifier to a specific game instance index.
    /// Use this function to set up which device controls which instance.
    pub fn map_device_to_instance_by_identifier(&mut self, identifier: DeviceIdentifier, instance_index: usize) -> Result<(), InputMuxError> {
        info!("Mapping device {:?} to instance index {}", identifier, instance_index);
        if self.devices.contains_key(&identifier) {
            if self.virtual_devices.contains_key(&instance_index) {
                self.instance_map.insert(identifier, instance_index);
                info!("Successfully mapped device {:?} to instance {}", identifier, instance_index);
                Ok(())
            } else {
                warn!("Virtual device for instance index {} not found. Cannot map device {:?}.", instance_index, identifier);
                Err(InputMuxError::GenericError(format!("Virtual device for instance {} not found", instance_index)))
            }
        } else {
            warn!("Physical input device {:?} not found among enumerated devices. Cannot map to instance {}. Available devices: {:?}", identifier, instance_index, self.devices.keys());
            Err(InputMuxError::DeviceNotFound(format!("{:?}", identifier))) // Return error with identifier info
        }
    }

    // You might want functions to get available devices and their identifiers
    pub fn get_available_devices(&self) -> Vec<DeviceIdentifier> {
        self.devices.keys().cloned().collect()
    }

     /// Gets the identifier for a device by its name. Returns the first match.
     /// Note: Use `get_available_devices` and match identifiers for robustness.
     pub fn get_device_identifier_by_name(&self, name: &str) -> Option<DeviceIdentifier> {
         self.devices.keys().find(|id| id.name == name).cloned()
     }

    /// Gets the system name (/dev/input/eventX) for the virtual device of a given instance.
    pub fn get_virtual_device_sysname(&self, instance_index: usize) -> Option<String> {
        self.virtual_devices.get(&instance_index)
            .and_then(|dev| dev.syspath())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }
    
    /// Get statistics about the input multiplexer
    pub fn get_stats(&self) -> InputMuxStats {
        InputMuxStats {
            total_devices: self.devices.len(),
            mapped_devices: self.instance_map.len(),
            virtual_devices: self.virtual_devices.len(),
            is_running: self.running.load(Ordering::SeqCst),
        }
    }
}

/// Statistics about the input multiplexer
#[derive(Debug, Clone)]
pub struct InputMuxStats {
    pub total_devices: usize,
    pub mapped_devices: usize,
    pub virtual_devices: usize,
    pub is_running: bool,
}

// Implement Drop to stop capture threads when InputMux goes out of scope
impl Drop for InputMux {
    fn drop(&mut self) {
        self.stop_capture();
        info!("InputMux instance dropped.");
    }
}


// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use env_logger; // Add env_logger = "0.11" to Cargo.toml

    // Helper to set up a basic logger for tests
    fn setup_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    // Basic test for enumeration (might require running with sufficient permissions)
    #[test]
    #[ignore] // Ignore this test by default as it requires special permissions/environment
    fn test_enumerate_devices() {
        setup_logger();
        let mut input_mux = InputMux::new();
        let result = input_mux.enumerate_devices();

        // Assert that enumeration either succeeded or failed with an expected permission error
        // The exact error kind can vary, so a general check for Ok or an appropriate Err is needed.
        if let Err(e) = result {
            eprintln!("Enumeration failed, potentially due to permissions: {}", e);
            // Depending on the test environment, you might assert specific error kinds
            // assert!(e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound || e.kind() == io::ErrorKind::Other);
             panic!("Enumeration failed: {}", e); // For test failure, panic with the error
        } else {
             // If successful, assert that some devices were found (if expected in the test env)
             // This requires a test environment with input devices available to the test process.
             info!("Successfully enumerated devices.");
             // assert!(!input_mux.devices.is_empty(), "No devices found, but enumeration succeeded.");
        }
    }

    // Add tests for creating virtual devices (requires /dev/uinput access)
     #[test]
     #[ignore] // Requires root or appropriate permissions for /dev/uinput
     fn test_create_virtual_devices() {
         setup_logger();
         let mut input_mux = InputMux::new();
         let num_instances = 3;
         let result = input_mux.create_virtual_devices(num_instances);

         if let Err(e) = result {
             eprintln!("Failed to create virtual devices, potentially due to permissions: {}", e);
             panic!("Failed to create virtual devices: {}", e);
         } else {
             info!("Successfully created virtual devices.");
             assert_eq!(input_mux.virtual_devices.len(), num_instances);
             for i in 0..num_instances {
                 assert!(input_mux.virtual_devices.contains_key(&i));
                 assert!(input_mux.get_virtual_device_sysname(i).is_some());
             }
         }
     }

    // Add tests for mapping devices and injecting events (requires complex setup)
    // These would likely require mocking evdev and uinput or running in a controlled environment.
    // #[test]
    // #[ignore]
    // fn test_mapping_and_injection() { ... }

     #[test]
     #[ignore] // Requires root or appropriate permissions for /dev/uinput and /dev/input
     fn test_stop_capture() {
         setup_logger();
         let mut input_mux = InputMux::new();

         // Dummy setup for testing stop_capture without real devices
         // In a real test, you'd enumerate real devices and create virtual ones.
         // For this test, we'll just simulate a running state.
         input_mux.running.store(true, Ordering::SeqCst);
         // We would ideally have a dummy capture thread running here that checks the flag.
         // Since we don't have a real device to read from easily in a test,
         // this test primarily checks the state change and join logic if threads were running.

         info!("Calling stop_capture...");
         input_mux.stop_capture();
         info!("stop_capture finished.");

         assert_eq!(input_mux.running.load(Ordering::SeqCst), false);
         assert!(input_mux.capture_threads.is_none()); // Handles should be consumed after joining
     }

     #[test]
     #[ignore] // Requires root or appropriate permissions for /dev/input
     fn test_map_device_by_name_and_identifier() {
         setup_logger();
         let mut input_mux = InputMux::new();

         // Enumerate devices to populate input_mux.devices
         if let Err(e) = input_mux.enumerate_devices() {
             eprintln!("Failed to enumerate devices for mapping test: {}", e);
             panic!("Failed to enumerate devices for mapping test: {}", e);
         }

         // Create virtual devices
         let num_instances = 2;
         if let Err(e) = input_mux.create_virtual_devices(num_instances) {
             eprintln!("Failed to create virtual devices for mapping test: {}", e);
             panic!("Failed to create virtual devices for mapping test: {}", e);
         }

         let available_devices = input_mux.get_available_devices();

         if available_devices.is_empty() {
             warn!("No devices available for mapping test. Skipping mapping assertions.");
             // The test will still pass if no devices are found, as there's nothing to map.
             // A more robust integration test would require a guaranteed input device.
         } else {
             // Test mapping by identifier (most robust)
             let first_device_identifier = available_devices[0].clone();
             let map_result_identifier = input_mux.map_device_to_instance_by_identifier(first_device_identifier.clone(), 0);
             assert!(map_result_identifier.is_ok(), "Failed to map device by identifier: {:?}", map_result_identifier.err());
             assert_eq!(input_mux.instance_map.get(&first_device_identifier), Some(&0));

             // Test mapping by name (less robust, only works if names are unique or we find the right one)
             // Use the name of the first device found
             let device_name = available_devices[0].name.clone();
             let map_result_name = input_mux.map_device_to_instance(&device_name, 1); // Map the same device by name to instance 1

             if input_mux.devices.keys().filter(|id| id.name == device_name).count() > 1 {
                  warn!("Multiple devices found with the name '{}'. Mapping by name is ambiguous.", device_name);
                  // If multiple devices have the same name, map_device_to_instance will map the first one it finds.
                  // We can't reliably assert which specific device was mapped by name in this case.
                  // The test should acknowledge this ambiguity or use a test environment with unique names.
             } else {
                  assert!(map_result_name.is_ok(), "Failed to map device by name: {:?}", map_result_name.err());
                  // After mapping by name to instance 1, the map entry for the first device might be updated
                  // depending on how find().cloned() behaves with duplicates.
                  // Let's re-check the mapping for the original identifier. It should now point to instance 1.
                  assert_eq!(input_mux.instance_map.get(&first_device_identifier), Some(&1));
             }

             // Test mapping a non-existent device by name
             let map_result_not_found = input_mux.map_device_to_instance("NonExistentDevice", 0);
             assert!(map_result_not_found.is_err());
             match map_result_not_found.unwrap_err() {
                 InputMuxError::DeviceNotFound(name) => assert_eq!(name, "NonExistentDevice"),
                 other => panic!("Expected DeviceNotFound error, but got {:?}", other),
             }

              // Test mapping to a non-existent instance index
              let map_result_no_virtual_device = input_mux.map_device_to_instance_by_identifier(first_device_identifier.clone(), num_instances + 1);
              assert!(map_result_no_virtual_device.is_err());
              match map_result_no_virtual_device.unwrap_err() {
                  InputMuxError::GenericError(msg) => assert!(msg.contains("Virtual device for instance")),
                  other => panic!("Expected GenericError about virtual device, but got {:?}", other),
              }
         }
     }
}

// The original main function is for testing the module independently.
// The actual application's main function is in src/main.rs.
// #[cfg(not(test))] // Compile this main only when not running tests
// fn main() {
//    // Initialize logger if running this module directly for testing
//    env_logger::init();

//    let mut input_mux = InputMux::new();

//    // Enumerate connected input devices
//    info!("Running InputMux test main.");
//    if let Err(e) = input_mux.enumerate_devices() {
//        eprintln!("Error enumerating devices: {}", e);
//        return;
//    }

//    // Define the number of instances you want to simulate
//    let num_instances = 2;

//    // Create virtual input devices for the instances
//    if let Err(e) = input_mux.create_virtual_devices(num_instances) {
//        eprintln!("Error creating virtual devices: {}", e);
//        return;
//    }


//    // Example mapping: Map the first two found devices to instances 0 and 1
//    let available_devices = input_mux.get_available_devices();
//    if available_devices.len() >= num_instances {
//        for i in 0..num_instances {
//            let device_identifier = available_devices[i].clone();
//            if let Err(e) = input_mux.map_device_to_instance_by_identifier(device_identifier, i) {
//                eprintln!("Error mapping device to instance {}: {}", i, e);
//            }
//        }
//    } else {
//        warn!("Not enough input devices found ({}) to map to {} instances.", available_devices.len(), num_instances);
//        // You might want to handle this case, e.g., exit or map only the available devices
//    }


//    // Capture raw input events from each mapped physical device and inject into virtual devices
//    if let Err(e) = input_mux.capture_events() {
//        error!("Error during input event capture: {}", e);
//    }

//    // Keep the main thread alive
//    info!("Input capture started. Main thread sleeping.");
//    // In a real application with a GUI, you would typically run the GUI event loop here.
//    // For this test, we'll just sleep or wait for a signal to stop.
//    // For demonstration of stopping, you could add a signal handler or a command input.

//    // Example: Sleep for a while then stop capture
//    thread::sleep(Duration::from_secs(30));
//    info!("Test duration elapsed. Attempting to stop capture.");
//    input_mux.stop_capture(); // Stop the capture threads

//    info!("Main thread exiting.");
// }
