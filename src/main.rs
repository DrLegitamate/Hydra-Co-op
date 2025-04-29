// Top-level flowchart (as commented ASCII art) that outlines the overall bootstrapping process

// +-----------------------------+
// | Initialize Logging System   |
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Parse Command-Line Arguments|
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Load User Configuration     |
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Launch Game Instances       |
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Set Up Virtual Network      |
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Adjust Windows              |
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Initialize Input Multiplexer|
// +-----------------------------+
//               |
//               v
// +-----------------------------+
// | Detect & Launch Proton Games| // This step is now integrated into Instance Launch
// +-----------------------------+

use crate::cli::parse_args;
use crate::config::{Config, ConfigError}; // Import ConfigError
use crate::instance_manager::{launch_multiple_game_instances, InstanceManagerError}; // Import InstanceManagerError
use crate::logging::init as init_logging; // Alias to avoid name conflict if another 'init' exists
use crate::net_emulator::{NetEmulator, NetEmulatorError}; // Import NetEmulatorError
// The top-level launch_game from proton_integration is removed/refactored,
// so we don't import it here anymore. Proton logic is called by instance_manager.
// use crate::proton_integration::launch_game;
use crate::window_manager::{WindowManager, Layout, WindowManagerError}; // Import WindowManagerError
use crate::input_mux::{InputMux, InputMuxError, DeviceIdentifier}; // Import InputMuxError and DeviceIdentifier
use std::{env, thread};
use log::{info, error, warn, debug}; // Import warn and debug for consistency
use std::path::{Path, PathBuf}; // Import Path and PathBuf
use clap::ArgMatches; // Import ArgMatches
use std::time::Duration;
use std::collections::HashMap; // Import HashMap
use std::process::Child; // Import Child if needed for instance management
use std::fs; // Import fs for creating WINEPREFIX base directory

fn main() {
    // Initialize the logging system first.
    // Configure the log level based on environment variables (e.g., RUST_LOG)
    // before calling init_logging().
    // The 'debug' command-line flag can set the RUST_LOG environment variable.
    let matches: ArgMatches = parse_args(); // Parse args early to check for debug flag

    let debug: bool = *matches.get_one("debug").unwrap_or(&false); // Get the debug flag

    if debug {
        // Set the RUST_LOG environment variable to enable debug logs
        env::set_var("RUST_LOG", "debug");
        info!("Debug mode enabled.");
    } else {
        // Set a default logging level (e.g., info) if not already set
        // env_logger::init() or your init_logging() typically reads RUST_LOG.
        // If RUST_LOG is not set, init() might default to Error or Info.
        // To explicitly set info unless RUST_LOG is already set:
        if env::var("RUST_LOG").is_err() {
             env::set_var("RUST_LOG", "info");
        }
         info!("Info mode enabled.");
    }

    // Now initialize the logging system.
    init_logging(); // Call your logging initialization function


    // Retrieve parsed command-line arguments using clap 4.0+ methods
    let game_executable_str: &String = matches.get_one("game_executable").expect("game_executable argument missing");
    let game_executable_path = Path::new(game_executable_str);

    let instances: u32 = *matches.get_one("instances").expect("instances argument missing");
    let instances_usize = instances as usize; // Convert to usize for collection sizes, loops, etc.

    // input_devices is collected as Vec<String> by default with ArgAction::Append in cli.rs
    // Collect it as Vec<&str> here for consistency with your original main.rs logic.
    let input_devices_arg: Vec<&str> = matches.get_many::<String>("input_devices")
        .expect("input_devices argument missing")
        .map(|s| s.as_str()) // Map &String to &str
        .collect();

    let layout_str: &String = matches.get_one("layout").expect("layout argument missing");
    let layout = Layout::from(layout_str.as_str()); // Use the From implementation for Layout

    let use_proton: bool = *matches.get_one("proton").unwrap_or(&false); // Assuming 'proton' is a boolean flag


    info!("Application started with the following arguments:");
    info!("  Game Executable: {}", game_executable_path.display());
    info!("  Number of Instances: {}", instances_usize);
    info!("  Input Devices: {:?}", input_devices_arg);
    info!("  Layout: {:?}", layout); // Use Debug print for Layout
    info!("  Debug Mode: {}", debug);
    info!("  Using Proton: {}", use_proton);


    // Load user configuration
    let config_path_str = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let config_path = Path::new(&config_path_str);
    info!("Attempting to load configuration from {}", config_path.display());

    // Use a match statement to handle the ConfigError from load()
    let config = match Config::load(config_path) {
        Ok(cfg) => {
            info!("Configuration loaded successfully from {}", config_path.display());
            cfg
        }
        Err(ConfigError::IoError(io_err)) => {
             if io_err.kind() == io::ErrorKind::NotFound {
                  warn!("Configuration file not found at {}. Using default configuration.", config_path.display());
                  // Return default configuration if the file is not found
                  Config::default_config()
             } else {
                  // Handle other IO errors
                 error!("Failed to load configuration from {}: I/O Error: {}", config_path.display(), io_err);
                 std::process::exit(1); // Fatal for other IO errors
             }
        }
        Err(ConfigError::TomlDeError(toml_err)) => {
            error!("Failed to parse configuration from {}: TOML Error: {}", config_path.display(), toml_err);
            // TOML parsing errors are usually fatal as the config is malformed
            std::process::exit(1);
        }
        Err(ConfigError::TomlSeError(_)) => {
             // Serialization errors should not happen during load, but handle defensively
             error!("Unexpected configuration serialization error during load from {}.", config_path.display());
             std::process::exit(1);
        }
         Err(ConfigError::GenericError(msg)) => {
             error!("Generic configuration error during load from {}: {}", config_path.display(), msg);
             std::process::exit(1);
         }
    };
    // You can now use the 'config' object. It will either be loaded from the file or the default.


    // Determine the base directory for WINEPREFIXes if using Proton.
    // This could be a temporary directory, a configured path, or derived from the game path.
    let base_wineprefix_dir = if use_proton {
        // Example: Use a directory in /tmp or a dedicated app data directory
         let mut dir = env::temp_dir(); // Start with the system's temporary directory
         dir.push("hydra_coop_wineprefixes"); // Add a subdirectory for the application
         // Consider making this configurable
         info!("Using base directory for WINEPREFIXes: {}", dir.display());

         // Ensure the base directory exists
         if let Err(e) = fs::create_dir_all(&dir) {
              error!("Failed to create base WINEPREFIX directory {}: {}", dir.display(), e);
              // This is a fatal error if we need to create WINEPREFIXes
              std::process::exit(1);
         }
         dir

    } else {
        // If not using Proton, the base_wineprefix_dir is not strictly needed
        // by launch_multiple_game_instances, but we pass a placeholder or
        // handle this case in instance_manager. Let's pass a dummy path.
        PathBuf::from("/dev/null") // Or a temporary directory that will be ignored
    };


    // Launch the required number of game instances
    // Pass the use_proton flag and the base_wineprefix_dir to instance_manager
    info!("Launching {} game instances using executable: {}", instances_usize, game_executable_path.display());
    let mut game_instances = match launch_multiple_game_instances(
        game_executable_path,
        instances_usize,
        use_proton,
        &base_wineprefix_dir,
    ) {
        Ok(children) => {
            info!("Successfully launched {} game instances.", children.len());
            children
        }
        Err(e) => {
            // Handle specific InstanceManager errors
            match e {
                InstanceManagerError::ProtonPathNotFound => {
                    error!("Failed to launch game instances: Proton was requested but not found. Please ensure Proton is installed and accessible.");
                }
                InstanceManagerError::ProtonError(proton_e) => {
                     error!("Failed to launch game instances due to Proton error: {}", proton_e);
                }
                InstanceManagerError::IoError(io_e) => {
                    error!("Failed to launch game instances due to I/O error: {}", io_e);
                }
                 InstanceManagerError::WindowsBinaryCheckError(check_e) => {
                      error!("Failed to launch game instances due to Windows binary check error: {}", check_e);
                 }
                InstanceManagerError::GenericError(msg) => {
                    error!("Failed to launch game instances: {}", msg);
                }
            }
            std::process::exit(1); // Exit on instance launch failure
        }
    };

    // Note: At this point, the game processes are started, but their windows
    // might not be immediately available. The window manager needs to wait
    // for the windows to be created and become visible before attempting to
    // manipulate them. The window_manager::set_layout includes a basic
    // retry mechanism, but for robustness, consider a more sophisticated
    // waiting strategy (e.g., polling with increasing delays or listening for X11 events).


    // Set up the virtual network emulator to connect these instances
    let mut net_emulator = NetEmulator::new(); // Assuming new() is fallible or returns a Result in the future
    info!("Initializing network emulator.");

    // Map to store emulator instance ID to its bound port (needed for SocketAddr mapping)
    let mut emulator_instance_ports: HashMap<u8, u16> = HashMap::new();

    // Add game instances to the network emulator.
    // Using a simple 0-based index for the emulator instance ID here for simplicity.
    // This emulator instance ID needs to be consistently associated with a specific
    // game process and how that game process identifies itself in the network.
    info!("Adding {} instances to network emulator.", game_instances.len());
    for (i, instance) in game_instances.iter().enumerate() {
        let emulator_instance_id = i as u8; // Using 0-based index as emulator instance ID
        let pid = instance.id(); // Get the PID of the launched process

        // Check if the emulator_instance_id is within the u8 range if required by add_instance
         if emulator_instance_id as u32 != i as u32 { // Check for overflow if i is large
              error!("Instance index {} exceeds u8 capacity for emulator ID. Cannot add to network emulator.", i);
              // Decide if this is a fatal error or if the application can continue
              // with fewer instances in the network emulator. For now, log and skip.
              continue;
         }


        match net_emulator.add_instance(emulator_instance_id) {
            Ok(bound_port) => {
                emulator_instance_ports.insert(emulator_instance_id, bound_port);
                info!("Instance {} (PID: {}) added to net emulator, bound to port {}", emulator_instance_id, pid, bound_port);
            }
            Err(e) => {
                 error!("Failed to add instance {} (PID: {}) to net emulator: {}", emulator_instance_id, pid, e);
                 // Decide if this failure is fatal. Logging and continuing might be acceptable.
                 // If a critical number of instances fail to add, you might exit.
                 // For now, log and continue, but the emulator won't handle this instance.
            }
        }
    }

     // TODO: Establish network mappings (src -> dst SocketAddr)
     // This is the challenging part and depends heavily on the target game's networking.
     // You need to determine the SocketAddrs that game instances will use to send
     // and receive packets and map them to the ports the emulator instances are bound to.
     //
     // Strategies could include:
     // 1. Configuring games to use specific, known ports and communicate on localhost.
     // 2. Having games report their network addresses to the launcher (e.g., via stdout/stderr or a file).
     // 3. More advanced techniques like intercepting network calls or using network namespaces (very complex).

     // Example (Illustrative - requires knowing game communication details):
     /*
     use std::net::SocketAddr;
     let game1_internal_addr: SocketAddr = "127.0.0.1:5000".parse().expect("Invalid game1 internal address");
     let game2_internal_addr: SocketAddr = "127.0.0.1:5001".parse().expect("Invalid game2 internal address");

     let emulator1_port = emulator_instance_ports.get(&0).expect("Emulator port for instance 0 not found");
     let emulator2_port = emulator_instance_ports.get(&1).expect("Emulator port for instance 1 not found");

     let emulator1_listening_addr: SocketAddr = format!("127.0.0.1:{}", emulator1_port).parse().expect("Invalid emulator1 listening address");
     let emulator2_listening_addr: SocketAddr = format!("127.0.0.1:{}", emulator2_port).parse().expect("Invalid emulator2 listening address");


     // Example Mapping Scenario: Games send to each other's internal addresses (5000, 5001) on localhost.
     // We need to redirect this traffic through the emulator sockets.
     // Packets FROM game1 (src: game1_internal_addr) should go TO emulator2's listening socket.
     net_emulator.add_mapping(game1_internal_addr, emulator2_listening_addr);
     // Packets FROM game2 (src: game2_internal_addr) should go TO emulator1's listening socket.
     net_emulator.add_mapping(game2_internal_addr, emulator1_listening_addr);

     warn!("Network mapping logic needs to be implemented based on target game's networking.");
     */


    // Start the network relay thread
    info!("Starting network emulator relay.");
    if let Err(e) = net_emulator.start_relay() {
         error!("Failed to start network emulator relay: {}", e);
         // Starting the relay is crucial for inter-instance communication. Likely fatal.
         std::process::exit(1);
    }
     info!("Network emulator relay started in background thread.");


    // Adjust the windows using the window management module to arrange them
    // in the selected split-screen layout.
    // This requires finding the windows associated with the launched processes.
    let window_manager = match WindowManager::new() {
        Ok(wm) => {
            info!("Window manager initialized successfully.");
            wm
        }
        Err(e) => {
            error!("Failed to initialize window manager: {}", e);
            // Window management is crucial for split-screen. This is likely fatal.
            std::process::exit(1);
        }
    };

    // Collect the PIDs of the launched game instances for the window manager
    let game_instance_pids: Vec<u32> = game_instances.iter().map(|instance| instance.id()).collect();
    info!("Attempting to set window layout for PIDs: {:?}", game_instance_pids);


    // Set the layout for the windows corresponding to the launched PIDs
    if let Err(e) = window_manager.set_layout(&game_instance_pids, layout) {
         // Use match for more specific error handling if needed
         match e {
             WindowManagerError::WindowNotFound => {
                 error!("Failed to set window layout: One or more game windows were not found after launch.");
                 // Decide if this is a fatal error. The games are running, but not arranged.
                 // You might inform the user and exit, or inform and let them manually arrange.
                 std::process::exit(1); // Keeping the exit pattern from original code
             }
             WindowManagerError::MonitorDetectionError(msg) => {
                  error!("Failed to set window layout: Monitor detection error: {}", msg);
                  std::process::exit(1);
             }
             other_error => {
                  error!("Failed to set window layout: An unexpected error occurred: {}", other_error);
                  std::process::exit(1);
             }
         }
    }
    info!("Window layout set successfully.");


    // Initialize the input multiplexer to route inputs from individual devices
    // to their assigned game instances.
    let mut input_mux = InputMux::new(); // Assuming InputMux::new() is fallible or returns a Result
    info!("Initializing input multiplexer.");

    // Enumerate connected input devices
    info!("Enumerating physical input devices.");
    if let Err(e) = input_mux.enumerate_devices() {
        match e {
            InputMuxError::IoError(io_e) => {
                 if io_e.kind() == io::ErrorKind::PermissionDenied {
                     error!("Permission denied when enumerating input devices. Run with sufficient privileges (e.g., add user to 'input' group or use sudo): {}", io_e);
                 } else {
                      error!("I/O error enumerating input devices: {}", io_e);
                 }
            }
            other_error => {
                error!("Failed to enumerate input devices: {}", other_error);
            }
        }
        // Cannot proceed without input devices. This is likely fatal.
        std::process::exit(1);
    }
    let available_devices = input_mux.get_available_devices();
    info!("Input devices enumerated. Found {} usable devices.", available_devices.len());
    debug!("Available devices: {:?}", available_devices);

    // Create virtual input devices for each game instance
    info!("Creating virtual input devices for {} instances.", instances_usize);
    if let Err(e) = input_mux.create_virtual_devices(instances_usize) {
        match e {
            InputMuxError::UinputError(uinput_e) => {
                 // Uinput creation often requires write permissions on /dev/uinput
                 if let Some(io_e) = uinput_e.source().and_then(|s| s.downcast_ref::<io::Error>()) {
                     if io_e.kind() == io::ErrorKind::PermissionDenied {
                         error!("Permission denied when creating virtual input devices. Run with sufficient privileges (e.g., add user to 'uinput' group or use sudo): {}", uinput_e);
                     } else {
                         error!("Uinput error creating virtual devices: {}", uinput_e);
                     }
                 } else {
                     error!("Failed to create virtual input devices: {}", other_error);
                 }
            }
            other_error => {
                error!("Failed to create virtual input devices: {}", other_error);
            }
        }
         // Cannot proceed without virtual devices for routing. This is fatal.
        std::process::exit(1);
    }
     info!("Virtual input devices created.");


    // Map input devices to instances based on command-line arguments
    if input_devices_arg.is_empty() {
         warn!("No input devices specified via command line ('-d' argument). Input multiplexing may not work as intended.");
         // Decide how to handle this: map defaults, show a GUI for mapping, or exit.
         // For now, we'll continue but input won't be routed unless default mapping exists in InputMux.
    } else {
         info!("Mapping specified input devices to instances.");

         if input_devices_arg.len() > instances_usize {
              warn!("More input devices specified ({}) than launched instances ({}). Extra devices will not be mapped.", input_devices_arg.len(), instances_usize);
         }

         // The order of devices in input_devices_arg corresponds to the instance index (0-based)
         for (instance_index, device_name_arg) in input_devices_arg.iter().enumerate() {
              if instance_index >= instances_usize {
                  info!("Skipping mapping for device '{}' as instance index {} is out of bounds.", device_name_arg, instance_index);
                  break; // Stop if we run out of instances to map to
              }

              // Find the DeviceIdentifier for the device name provided in arguments
              let device_identifier_option = available_devices.iter()
                   .find(|id| id.name == *device_name_arg)
                   .cloned(); // Clone the identifier to pass to map_device_to_instance_by_identifier

              match device_identifier_option {
                   Some(identifier) => {
                        // Map the found physical device identifier to the current instance index
                        if let Err(e) = input_mux.map_device_to_instance_by_identifier(identifier, instance_index) {
                             error!("Failed to map device '{}' to instance {}: {}", device_name_arg, instance_index, e);
                             // Decide if a mapping failure is fatal. Logging and continuing might be acceptable
                             // if other devices are successfully mapped.
                        } else {
                             info!("Successfully mapped device '{}' to instance {}.", device_name_arg, instance_index);
                        }
                   }
                   None => {
                        warn!("Specified input device '{}' not found among available devices. Cannot map to instance {}. Available devices: {:?}", device_name_arg, instance_index, available_devices);
                        // Continue to the next specified device argument
                   }
              }
         }
    }


    // Capture input events. This will spawn threads and keep the application running.
    // This function should ideally return join handles for the spawned threads
    // or signal readiness for the main thread to wait.
    info!("Starting input event capture and routing.");
    if let Err(e) = input_mux.capture_events() {
         error!("Error during input event capture setup: {}", e);
         // If capture setup fails, input won't work. Likely fatal.
         std::process::exit(1);
    }
    info!("Input event capture started. Background threads are running.");


    // The main thread needs to stay alive to keep the background threads
    // (input capture, network emulator) running.
    // If you had a GUI, its event loop would go here.
    // For a console application, you can wait on the spawned threads
    // or simply enter a blocking state.

    // To wait for the input capture threads to finish (e.g., on graceful shutdown):
    // You would need capture_events to return Vec<thread::JoinHandle<Result<(), InputMuxError>>>.
    // Then, iterate through the handles and call .join().

    info!("Hydra Co-op is running. Background services started.");
    info!("Press Ctrl+C to initiate shutdown.");

    // A simple way to keep the main thread alive in a console app is to wait
    // on a signal (like Ctrl+C) or enter a long-running operation.
    // For graceful shutdown, you would need signal handling (e.g., using the `ctrlc` crate).

    // Example using the 'ctrlc' crate (add `ctrlc = "3.2"` to Cargo.toml):
    /*
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Ctrl+C received. Initiating graceful shutdown.");
        r.store(Ordering::SeqCst, Ordering::SeqCst); // Use consistent ordering
    }).expect("Error setting Ctrl-C handler");

    // Wait until Ctrl+C is pressed
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }

    info!("Shutdown sequence started. Waiting for background tasks...");
    // Here, you would signal your background threads (input_mux, net_emulator) to stop
    // and then wait for them to join.
    // (Requires modification to capture_events and NetEmulator to return JoinHandles
    // and accept stop signals in their threads).

    // For example, if NetEmulator::start_relay returned a JoinHandle and stop_relay worked:
    // if let Some(net_emulator_thread) = net_emulator.relay_thread.take() { // Requires relay_thread to be public or have a getter
    //      if let Err(e) = net_emulator.stop_relay() { // Assuming stop_relay is still needed for signaling
    //           error!("Error stopping network relay: {}", e);
    //      }
    //      match net_emulator_thread.join() {
    //           Ok(thread_result) => {
    //                if let Err(e) = thread_result {
    //                     error!("Network relay thread finished with error: {}", e);
    //                } else {
    //                     info!("Network relay thread joined successfully.");
    //                }
    //           }
    //           Err(e) => error!("Network relay thread panicked: {:?}", e),
    //      }
    // }

    // Similar logic for InputMux capture threads if they returned JoinHandles

    info!("Background tasks finished. Exiting.");
    */

    // Without explicit signal handling and graceful shutdown for threads,
    // the simplest approach is a long-running loop or relying on the OS
    // to clean up resources on process exit.

    // Simple blocking loop if no graceful shutdown is implemented yet:
     loop {
         // This loop keeps the main thread alive.
         // In a real application, this would be a GUI event loop
         // or a more sophisticated waiting/shutdown mechanism.
         thread::sleep(Duration::from_secs(60)); // Sleep for a minute to reduce CPU usage
     }

    // Code after the loop will only be reached if the loop breaks (e.g., via graceful shutdown)
    // info!("Application finished."); // This line might not be reached in a simple blocking loop

}

// Note: Ensure all modules used here (cli, config, instance_manager, logging,
// net_emulator, proton_integration, window_manager, input_mux) exist and
// have the expected public functions and types as called in main.rs.
// Remember to add necessary dependencies (like uinput, polling, tempfile, ctrlc) to your Cargo.toml.
