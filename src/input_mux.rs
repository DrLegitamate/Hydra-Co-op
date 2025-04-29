use evdev::{Device, InputEvent, InputEventKind, ReadFlag};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write}; // Import Read and Write
use std::path::Path;
use std::env;
use std::sync::{Arc, RwLock};
use log::{info, warn, error, debug}; // Import debug log level
use std::thread;
use std::time::Duration;

// We will use the uinput-rs crate for creating virtual input devices.
// Add this to your Cargo.toml:
// [dependencies]
// uinput = "0.5" # Or the latest version


// Custom error type for input multiplexing operations
#[derive(Debug)]
pub enum InputMuxError {
    IoError(io::Error),
    EvdevError(evdev::Error),
    UinputError(uinput::Error),
    DeviceNotFound(String),
    MissingDeviceInfo,
    GenericError(String),
}

impl std::fmt::Display for InputMuxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InputMuxError::IoError(e) => write!(f, "I/O error: {}", e),
            InputMuxError::EvdevError(e) => write!(f, "evdev error: {}", e),
            InputMuxError::UinputError(e) => write!(f, "uinput error: {}", e),
            InputMuxError::DeviceNotFound(name) => write!(f, "Input device not found: {}", name),
            InputMuxError::MissingDeviceInfo => write!(f, "Missing device information"),
            InputMuxError::GenericError(msg) => write!(f, "Input multiplexer error: {}", msg),
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
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct DeviceIdentifier {
    name: String,
    phys: Option<String>, // Physical port/bus
    bustype: u16,
    vendor_id: u16,
    product_id: u16,
    version: u16,
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
    // We no longer need active_instances in this simplified routing model
    // active_instances: Arc<RwLock<HashMap<usize, bool>>>,
}

impl InputMux {
    pub fn new() -> Self {
        InputMux {
            devices: HashMap::new(),
            instance_map: HashMap::new(),
            virtual_devices: HashMap::new(),
            // active_instances: Arc::new(RwLock::new(HashMap::new())),
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


        for entry in fs::read_dir(input_dir)? {
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
                         warn!("Failed to open device {}: {}", path.display(), e);
                        // Continue to the next device
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
         // You might want to configure the capabilities of the virtual device
         // based on the capabilities of the physical devices you want to multiplex.
         // For simplicity, this example creates a generic keyboard and mouse virtual device.

         // TODO: Configure virtual device capabilities based on collected physical device capabilities.
         let mut capabilities = uinput::Builder::new()?
             .name("HydraCoop Virtual Device")?
             .event(uinput::event::Relative::Relative)?; // Example: Enable relative motion events

         // Example: Enable some keyboard events
         for key in uinput::event::Key::iter() {
             capabilities = capabilities.event(uinput::event::Key::new(*key))?;
         }

         // You would need to add more events based on the devices you support (e.g., mouse buttons, joysticks)


        for i in 0..num_instances {
            // Create a unique name for each virtual device instance
            let device_name = format!("HydraCoop_Virtual_Device_{}", i);
             debug!("Creating virtual device: {}", device_name);

            let virtual_device = uinput::Builder::new()?
                .name(&device_name)?
                // Inherit or set capabilities based on actual connected devices or desired features
                 .event(uinput::event::Relative::Relative)? // Example capability
                 .event(uinput::event::Key::Enter)? // Example capability
                // Add more capabilities based on requirements
                .create()?;

             info!("Created virtual device for instance {}: {}", i, device_name);
            self.virtual_devices.insert(i, virtual_device);
        }

         info!("Finished creating virtual devices. Created {} devices.", self.virtual_devices.len());
        Ok(())
    }


    /// Maps a physical input device to a specific game instance.
    /// The device is identified by its name (this might need refinement for uniqueness).
    pub fn map_device_to_instance(&mut self, device_name: &str, instance_index: usize) -> Result<(), InputMuxError> {
        info!("Attempting to map device '{}' to instance index {}", device_name, instance_index);

        // Find the DeviceIdentifier for the given device name
        let device_identifier = self.devices.keys()
            .find(|id| id.name == device_name)
            .cloned(); // Clone the identifier to use as a key

        match device_identifier {
            Some(identifier) => {
                 if self.virtual_devices.contains_key(&instance_index) {
                      info!("Mapping physical device '{}' ({:?}) to virtual device for instance {}.", device_name, identifier, instance_index);
                     self.instance_map.insert(identifier, instance_index);
                     Ok(())
                 } else {
                      warn!("Virtual device for instance index {} not found. Cannot map device '{}'.", instance_index, device_name);
                      Err(InputMuxError::GenericError(format!("Virtual device for instance {} not found", instance_index)))
                 }
            }
            None => {
                warn!("Physical input device '{}' not found. Cannot map to instance {}. Available devices: {:?}", device_name, instance_index, self.devices.keys());
                Err(InputMuxError::DeviceNotFound(device_name.to_string()))
            }
        }
    }


    /// Captures events from mapped physical devices and injects them into the
    /// corresponding virtual devices for each instance.
    /// This function typically runs indefinitely in separate threads.
    pub fn capture_events(&self) -> Result<(), InputMuxError> {
        info!("Starting input event capture and routing...");

        if self.devices.is_empty() {
             warn!("No input devices enumerated. Skipping event capture.");
             return Ok(()); // Or return an error if no devices is considered a fatal issue
        }

        if self.virtual_devices.is_empty() {
             error!("No virtual devices created. Cannot route input events.");
             return Err(InputMuxError::GenericError("No virtual devices available for routing".to_string()));
        }


        let mut join_handles = Vec::new();

        for (identifier, device) in &self.devices {
            let device = device.clone(); // Clone the device for the thread
            let identifier = identifier.clone(); // Clone the identifier
            let instance_map = self.instance_map.clone(); // Clone the map for the thread
            let virtual_devices = self.virtual_devices.clone(); // Clone the map of virtual devices

            info!("Starting capture thread for device: {}", identifier.name);
            let handle = thread::spawn(move || {
                loop {
                    // Read the next input event from the physical device
                    match device.next_event(ReadFlag::NORMAL) {
                        Ok(Some(event)) => {
                            debug!("Captured event from device '{}': {:?}", identifier.name, event);

                            // Find which instance this device is mapped to
                            if let Some(&instance_index) = instance_map.get(&identifier) {
                                // Get the virtual device for the target instance
                                if let Some(virtual_device) = virtual_devices.get(&instance_index) {
                                    // Inject the event into the virtual device
                                     debug!("Injecting event to virtual device for instance {}: {:?}", instance_index, event);
                                    if let Err(e) = virtual_device.write_event(&event) {
                                         error!("Failed to inject event for device '{}' to instance {}: {}", identifier.name, instance_index, e);
                                        // Depending on the error, you might want to break the loop or handle it differently
                                    } else {
                                        // Sync the virtual device after injecting events (especially button/key events)
                                        if event.kind() == InputEventKind::Key || event.kind() == InputEventKind::Button {
                                             if let Err(e) = virtual_device.synchronize() {
                                                  error!("Failed to synchronize virtual device for instance {}: {}", instance_index, e);
                                             }
                                        }
                                    }
                                } else {
                                     warn!("No virtual device found for instance index {} mapped to device '{}'. Event not routed.", instance_index, identifier.name);
                                }
                            } else {
                                 debug!("Device '{}' ({:?}) is not mapped to any instance. Event not routed.", identifier.name, identifier);
                            }
                        }
                        Ok(None) => {
                            // No event available, continue or sleep briefly
                             debug!("No event from device '{}', sleeping.", identifier.name);
                             thread::sleep(Duration::from_millis(10)); // Prevent busy-waiting
                        }
                        Err(e) => {
                            // Handle errors reading from the device (e.g., device disconnected)
                            error!("Error reading event from device '{}' ({:?}): {}", identifier.name, identifier, e);
                            // Depending on the error kind, you might break the loop to stop processing this device
                             if e.kind() == io::ErrorKind::BrokenPipe || e.kind() == io::ErrorKind::NotFound {
                                 warn!("Device '{}' appears disconnected. Stopping capture for this device.", identifier.name);
                                 break; // Stop the thread for this device
                             }
                            // For other errors, you might log and continue, or implement a retry mechanism
                        }
                    }
                }
                 info!("Capture thread for device '{}' exited.", identifier.name);
            });
            join_handles.push(handle);
        }

        // Note: The main thread will likely need to wait on these join_handles
        // or the application will exit when main() finishes.
        // A common pattern is to move this into a function that is called by main,
        // and main then waits or enters its own event loop (e.g., for the GUI).

        // For now, just return Ok and let the main thread in main.rs decide how to wait.
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
              warn!("Physical input device {:?} not found. Cannot map to instance {}. Available devices: {:?}", identifier, instance_index, self.devices.keys());
             Err(InputMuxError::DeviceNotFound(format!("{:?}", identifier))) // Return error with identifier info
         }
    }

    // You might want functions to get available devices and their identifiers
    pub fn get_available_devices(&self) -> Vec<DeviceIdentifier> {
        self.devices.keys().cloned().collect()
    }
}

// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    // Note: Testing low-level input handling requires either root permissions
    // or specific user group memberships, and potentially setting up dummy
    // input devices or a virtual environment. These tests are often
    // more complex integration tests than simple unit tests.

    // Basic test for enumeration (might require running with sufficient permissions)
    #[test]
    #[ignore] // Ignore this test by default as it requires special permissions/environment
    fn test_enumerate_devices() {
        let mut input_mux = InputMux::new();
        let result = input_mux.enumerate_devices();

        // Assert that enumeration either succeeded or failed with an expected permission error
        // The exact error kind can vary, so a general check for Ok or an appropriate Err is needed.
        if let Err(e) = result {
            // Check if the error is likely due to permissions or device access
            eprintln!("Enumeration failed, potentially due to permissions: {}", e);
            // Depending on the test environment, you might assert specific error kinds
            // assert!(e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound || e.kind() == io::ErrorKind::Other);
             panic!("Enumeration failed: {}", e); // For test failure, panic with the error
        } else {
             // If successful, assert that some devices were found (if expected in the test env)
             // assert!(!input_mux.devices.is_empty(), "No devices found, but enumeration succeeded.");
              info!("Successfully enumerated devices.");
        }
    }

    // Add tests for creating virtual devices (requires /dev/uinput access)
    // #[test]
    // #[ignore]
    // fn test_create_virtual_devices() { ... }

    // Add tests for mapping devices and injecting events (requires complex setup)
    // #[test]
    // #[ignore]
    // fn test_mapping_and_injection() { ... }
}

// The original main function is for testing the module independently.
// The actual application's main function is in src/main.rs.
// #[cfg(not(test))] // Compile this main only when not running tests
// fn main() {
//     // Initialize logger if running this module directly for testing
//     // env_logger::init();

//     let mut input_mux = InputMux::new();

//     // Enumerate connected input devices
//     info!("Running InputMux test main.");
//     if let Err(e) = input_mux.enumerate_devices() {
//         eprintln!("Error enumerating devices: {}", e);
//         return;
//     }

//     // Define the number of instances you want to simulate
//     let num_instances = 2;

//     // Create virtual input devices for the instances
//     if let Err(e) = input_mux.create_virtual_devices(num_instances) {
//         eprintln!("Error creating virtual devices: {}", e);
//         return;
//     }


//     // Example mapping: Map the first two found devices to instances 0 and 1
//     let available_devices = input_mux.get_available_devices();
//     if available_devices.len() >= num_instances {
//         for i in 0..num_instances {
//             let device_identifier = available_devices[i].clone();
//             if let Err(e) = input_mux.map_device_to_instance_by_identifier(device_identifier, i) {
//                 eprintln!("Error mapping device to instance {}: {}", i, e);
//             }
//         }
//     } else {
//         warn!("Not enough input devices found ({}) to map to {} instances.", available_devices.len(), num_instances);
//         // You might want to handle this case, e.g., exit or map only the available devices
//     }


//     // Capture raw input events from each mapped physical device and inject into virtual devices
//     if let Err(e) = input_mux.capture_events() {
//          error!("Error during input event capture: {}", e);
//     }

//     // Keep the main thread alive
//     info!("Input capture started. Main thread sleeping.");
//     loop {
//         thread::sleep(Duration::from_secs(10)); // Sleep longer
//     }
// }
