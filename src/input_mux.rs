use evdev::Device;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::Path;
use std::env;
use std::sync::{Arc, Mutex};
use log::{info, warn, error, debug};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};

/// Custom error type for input multiplexing operations.
#[derive(Debug)]
pub enum InputMuxError {
    IoError(io::Error),
    EvdevError(evdev::Error),
    GenericError(String),
    AlreadyRunning,
}

impl std::fmt::Display for InputMuxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InputMuxError::IoError(e) => write!(f, "I/O error: {}", e),
            InputMuxError::EvdevError(e) => write!(f, "evdev error: {}", e),
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
            phys: device.physical_path().map(|s| s.to_string()),
            bustype: input_id.bus_type().0,
            vendor_id: input_id.vendor(),
            product_id: input_id.product(),
            version: input_id.version(),
        }
    }
}


/// Per-thread capture loop. Owns one physical Device, polls its fd in level-triggered
/// mode so the loop can wake on events without busy-spinning, then forwards each
/// fetched event to the virtual device for the assigned instance.
fn run_capture_loop(
    mut device: Device,
    identifier: DeviceIdentifier,
    instance_index: usize,
    virtual_devices: HashMap<usize, Arc<Mutex<VirtualDevice>>>,
    running_flag: Arc<std::sync::atomic::AtomicBool>,
) {
    let vd_arc = match virtual_devices.get(&instance_index) {
        Some(arc) => arc.clone(),
        None => {
            error!("Capture thread: virtual device for instance {} not found. Exiting thread for device '{}'.", instance_index, identifier.name);
            return;
        }
    };

    let poller = match polling::Poller::new() {
        Ok(p) => p,
        Err(e) => {
            error!("Capture thread for '{}': failed to create poller: {}", identifier.name, e);
            return;
        }
    };
    // SAFETY: we delete the device from the poller before dropping it (at thread exit
    // when `device` drops, after the loop returns and `poller` is dropped).
    if let Err(e) = unsafe {
        poller.add_with_mode(
            &device,
            polling::Event::readable(0),
            polling::PollMode::Level,
        )
    } {
        error!("Capture thread for '{}': failed to register device with poller: {}", identifier.name, e);
        return;
    }

    let mut events = polling::Events::new();
    let wait_timeout = Duration::from_millis(100);

    while running_flag.load(Ordering::SeqCst) {
        events.clear();
        match poller.wait(&mut events, Some(wait_timeout)) {
            Ok(0) => continue,
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                error!("Capture thread for '{}': poller error: {}", identifier.name, e);
                break;
            }
        }

        match device.fetch_events() {
            Ok(iter) => {
                let batch: Vec<_> = iter.collect();
                if batch.is_empty() {
                    continue;
                }
                let mut vd = vd_arc.lock().unwrap();
                if let Err(e) = vd.emit(&batch) {
                    error!("Failed to inject events for '{}' to instance {}: {}", identifier.name, instance_index, e);
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        error!("Broken pipe on virtual device for instance {}. Stopping capture for '{}'.", instance_index, identifier.name);
                        break;
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                error!("Error reading events from device '{}': {}", identifier.name, e);
                if matches!(e.kind(), io::ErrorKind::BrokenPipe | io::ErrorKind::NotFound) {
                    warn!("Device '{}' appears disconnected. Stopping capture for this device.", identifier.name);
                }
                break;
            }
        }
    }

    // Required by Poller's safety contract: deregister before the device fd is dropped.
    // SAFETY: the device is still alive at this point and its fd is still valid.
    let fd = unsafe { BorrowedFd::borrow_raw(device.as_raw_fd()) };
    let _ = poller.delete(fd);
    info!("Capture thread for device '{}' exited.", identifier.name);
}

pub struct InputMux {
    // Map DeviceIdentifier to the opened evdev::Device
    devices: HashMap<DeviceIdentifier, Device>,
    // Map DeviceIdentifier to the instance index (0, 1, 2...)
    instance_map: HashMap<DeviceIdentifier, usize>,
    // Map instance index to its virtual uinput device (Arc+Mutex for cross-thread access)
    virtual_devices: HashMap<usize, Arc<Mutex<VirtualDevice>>>,
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

    /// Creates virtual uinput devices for each game instance using evdev's built-in
    /// VirtualDeviceBuilder.  Each device mirrors the union of capabilities from all
    /// enumerated physical devices so that every key, axis, and button works in-game.
    /// Requires write permissions on /dev/uinput.
    pub fn create_virtual_devices(&mut self, num_instances: usize) -> Result<(), InputMuxError> {
        info!("Creating {} virtual input device(s)...", num_instances);
        self.virtual_devices.clear();

        // --- collect the union of all physical-device capabilities ---
        let mut all_keys: Vec<evdev::Key> = Vec::new();
        let mut all_rel_axes: Vec<evdev::RelativeAxisType> = Vec::new();
        let mut all_abs_axes: Vec<(evdev::AbsoluteAxisType, evdev::AbsInfo)> = Vec::new();

        for (_, device) in &self.devices {
            if let Some(keys) = device.supported_keys() {
                for key in keys.iter() {
                    if !all_keys.contains(&key) {
                        all_keys.push(key);
                    }
                }
            }
            if let Some(axes) = device.supported_relative_axes() {
                for axis in axes.iter() {
                    if !all_rel_axes.contains(&axis) {
                        all_rel_axes.push(axis);
                    }
                }
            }
            if let Some(axes) = device.supported_absolute_axes() {
                for axis in axes.iter() {
                    let already = all_abs_axes.iter().any(|(a, _)| *a == axis);
                    if !already {
                        // Use a safe generic range that covers all common gamepads/sticks.
                        let abs_info = evdev::AbsInfo::new(0, -32767, 32767, 16, 128, 1);
                        all_abs_axes.push((axis, abs_info));
                    }
                }
            }
        }

        info!(
            "Capabilities collected: {} keys, {} relative axes, {} absolute axes",
            all_keys.len(), all_rel_axes.len(), all_abs_axes.len()
        );

        let has_real_caps =
            !all_keys.is_empty() || !all_rel_axes.is_empty() || !all_abs_axes.is_empty();

        // --- create one virtual device per instance ---
        for i in 0..num_instances {
            let device_name = format!("HydraCoop Virtual Device {}", i);
            debug!("Creating virtual device: {}", device_name);

            let mut builder = VirtualDeviceBuilder::new()
                .map_err(InputMuxError::IoError)?
                .name(&device_name);

            if has_real_caps {
                if !all_keys.is_empty() {
                    let mut key_set = evdev::AttributeSet::<evdev::Key>::new();
                    for &k in &all_keys {
                        key_set.insert(k);
                    }
                    builder = builder.with_keys(&key_set)
                        .map_err(InputMuxError::IoError)?;
                }
                if !all_rel_axes.is_empty() {
                    let mut rel_set = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
                    for &a in &all_rel_axes {
                        rel_set.insert(a);
                    }
                    builder = builder.with_relative_axes(&rel_set)
                        .map_err(InputMuxError::IoError)?;
                }
                for &(axis, abs_info) in &all_abs_axes {
                    let setup = evdev::UinputAbsSetup::new(axis, abs_info);
                    builder = builder.with_absolute_axis(&setup)
                        .map_err(InputMuxError::IoError)?;
                }
            } else {
                // No physical devices enumerated yet — register a safe minimum so the
                // virtual device can at least accept common keyboard/mouse events.
                warn!("No physical device capabilities found; virtual device {} will use a default capability set.", i);
                let mut key_set = evdev::AttributeSet::<evdev::Key>::new();
                key_set.insert(evdev::Key::KEY_ENTER);
                key_set.insert(evdev::Key::KEY_SPACE);
                builder = builder.with_keys(&key_set)
                    .map_err(InputMuxError::IoError)?;
                let mut rel_set = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
                rel_set.insert(evdev::RelativeAxisType::REL_X);
                rel_set.insert(evdev::RelativeAxisType::REL_Y);
                builder = builder.with_relative_axes(&rel_set)
                    .map_err(InputMuxError::IoError)?;
            }

            let virtual_device = builder.build().map_err(InputMuxError::IoError)?;
            info!("Created virtual device for instance {}", i);
            self.virtual_devices.insert(i, Arc::new(Mutex::new(virtual_device)));
        }

        info!("Finished creating virtual devices ({} created).", self.virtual_devices.len());
        Ok(())
    }


    /// Captures events from mapped physical devices and injects them into the
    /// corresponding virtual devices for each instance.
    /// This function spawns a thread for each mapped physical device.
    pub fn capture_events(&mut self, assignments: &[(usize, InputAssignment)]) -> Result<(), InputMuxError> {
        // Clear existing mappings
        self.instance_map.clear();
        
        // Process input assignments
        let auto_detect_queue: Vec<DeviceIdentifier> = self.devices.keys().cloned().collect();
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
        self.running.store(true, Ordering::SeqCst);

        let mut join_handles = Vec::new();

        // Take ownership of mapped devices for their capture threads. evdev's Device
        // is not Clone and fetch_events requires &mut self, so each thread must own
        // its physical device exclusively. Unmapped devices remain in self.devices.
        let mapped_identifiers: Vec<DeviceIdentifier> = self.instance_map.keys().cloned().collect();
        for identifier in mapped_identifiers {
            let instance_index = match self.instance_map.get(&identifier).copied() {
                Some(i) => i,
                None => continue,
            };
            let device = match self.devices.remove(&identifier) {
                Some(d) => d,
                None => {
                    error!("Mapped device identifier {:?} not found among enumerated devices.", identifier);
                    continue;
                }
            };

            let virtual_devices = self.virtual_devices.clone();
            let running_flag = self.running.clone();
            let id_for_thread = identifier.clone();

            info!("Starting capture thread for device: {} (mapped to instance {})", id_for_thread.name, instance_index);

            let handle = thread::spawn(move || {
                run_capture_loop(device, id_for_thread, instance_index, virtual_devices, running_flag);
            });
            join_handles.push(handle);
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

    /// List of enumerated input devices that are currently available.
    pub fn get_available_devices(&self) -> Vec<DeviceIdentifier> {
        self.devices.keys().cloned().collect()
    }
}

// Implement Drop to stop capture threads when InputMux goes out of scope
impl Drop for InputMux {
    fn drop(&mut self) {
        if let Err(e) = self.stop_capture() {
            warn!("Error stopping input capture during drop: {}", e);
        }
        info!("InputMux instance dropped.");
    }
}


// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;

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
         let _ = input_mux.stop_capture();
         info!("stop_capture finished.");

         assert_eq!(input_mux.running.load(Ordering::SeqCst), false);
         assert!(input_mux.capture_threads.is_none()); // Handles should be consumed after joining
     }

}