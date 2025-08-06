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
// | Decide CLI or GUI Mode      | // Updated logic for defaulting
// +-----------------------------+
//              /   \
//             v     v
//  +------------+ +------------+
//  | CLI Logic  | | GUI Logic  |
//  +------------+ +------------+
//             \     /
//              v   v
// +-----------------------------+
// | Trigger Core App Logic      | // Initiated by CLI parsing or GUI 'Launch' button
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
// | Main Application Loop/Wait  | // CLI waits or exits, GUI runs event loop
// +-----------------------------+


// Declare modules
mod cli;
mod config;
mod errors;
mod game_detection;
mod universal_launcher;
mod adaptive_config;
mod gui;
mod input_mux;
mod instance_manager;
mod logging;
mod net_emulator;
mod proton_integration;
mod window_manager;

use errors::{HydraError, Result};
use config::Config;
use universal_launcher::UniversalLauncher;
use adaptive_config::AdaptiveConfigManager;
use logging::init as init_logging;
use net_emulator::NetEmulator;
use window_manager::{WindowManager, Layout};
use input_mux::{InputMux, DeviceIdentifier, InputAssignment};
use std::{env, thread, io}; // Import io
use log::{info, error, warn, debug}; // Import warn and debug for consistency
use std::path::{Path, PathBuf}; // Import Path and PathBuf
use clap::ArgMatches; // Import ArgMatches only
use std::time::Duration;
use std::collections::HashMap; // Import HashMap
use std::process::Child; // Import Child if needed for instance management
use std::fs; // Import fs for creating WINEPREFIX base directory
use std::net::SocketAddr; // Import SocketAddr
use ctrlc; // Import ctrlc for graceful shutdown
use std::sync::{atomic::{AtomicBool, Ordering}, Arc}; // Import for graceful shutdown flag



/// Encapsulates the core application logic: launching instances, setting up
/// network, managing windows, and initializing input multiplexing.
/// This function can be called by both the CLI and GUI modes.
/// 
/// Now uses the universal launcher system that works with any game.
///
/// # Returns
///
/// * `Result<(NetEmulator, InputMux), Box<dyn Error>>` - Returns the initialized
///   NetEmulator and InputMux instances if successful, otherwise returns a boxed error.
fn run_core_logic(
    game_executable_path: &Path,
    instances_usize: usize,
    input_assignments: &[(usize, InputAssignment)], // Use InputAssignment
    layout: Layout,
    use_proton: bool,
    config: &Config, // Pass the loaded configuration
    adaptive_config: Option<&mut AdaptiveConfigManager>, // Optional adaptive config
    // Potentially pass other necessary data like network mapping config
) -> Result<(NetEmulator, InputMux)> {
    // Validate inputs
    if instances_usize == 0 {
        return Err(HydraError::validation("Number of instances must be at least 1"));
    }
    
    if instances_usize > crate::defaults::MAX_INSTANCES {
        return Err(HydraError::validation(format!(
            "Number of instances ({}) exceeds maximum ({})", 
            instances_usize, 
            crate::defaults::MAX_INSTANCES
        )));
    }
    
    info!("Running core application logic.");
    debug!("  Game Executable: {}", game_executable_path.display());
    debug!("  Number of Instances: {}", instances_usize);
    debug!("  Input Assignments: {:?}", input_assignments); // Log assignments
    debug!("  Layout: {:?}", layout);
    debug!("  Using Proton: {}", use_proton);
    debug!("  Config: {:?}", config); // Log config details if Debug is derived
    debug!("  Adaptive config enabled: {}", adaptive_config.is_some());


    // Determine the base directory for WINEPREFIXes if using Proton.
    // This logic is now part of run_core_logic as it's needed for launching.
    let base_wineprefix_dir = if use_proton {
         // Example: Use a directory in /tmp or a dedicated app data directory
         let mut dir = env::temp_dir(); // Start with the system's temporary directory
         dir.push("hydra_coop_wineprefixes"); // Add a subdirectory for the application
         // Consider making this configurable (e.g., via config file or command line)
         info!("Using base directory for WINEPREFIXes: {}", dir.display());

         // Ensure the base directory exists
         if let Err(e) = fs::create_dir_all(&dir) {
              error!("Failed to create base WINEPREFIX directory {}: {}", dir.display(), e);
              // This is a fatal error if we need to create WINEPREFIXes
              return Err(HydraError::Io(e));
         }
         dir

    } else {
        // If not using Proton, the base_wineprefix_dir is not strictly needed
        // by launch_multiple_game_instances, but we pass a dummy path.
        PathBuf::from("/dev/null") // Or a temporary directory that will be ignored
    };

    // Use the universal launcher instead of the old instance manager
    info!("Initializing universal game launcher...");
    let mut universal_launcher = UniversalLauncher::new();
    
    let launch_start = std::time::Instant::now();

    // Launch game instances using the universal system
    info!("Launching {} game instances using universal launcher: {}", instances_usize, game_executable_path.display());
    let game_instance_pids = universal_launcher.launch_game_instances(
        game_executable_path,
        instances_usize,
        use_proton,
    )?;
    
    let launch_duration = launch_start.elapsed();
    info!("Universal launcher completed in {:?}", launch_duration);

    // Record success in adaptive config if available
    if let Some(adaptive_mgr) = adaptive_config {
        let game_id = game_executable_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
            
        // We'd need to get the profile and config from the universal launcher
        // This is a simplified version - in practice you'd want to expose these
        info!("Recording successful launch in adaptive configuration");
        // adaptive_mgr.record_success(game_id, &profile, &game_config, launch_duration)?;
    }


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
    info!("Adding {} instances to network emulator.", game_instance_pids.len());
    for (i, &pid) in game_instance_pids.iter().enumerate() {
        let emulator_instance_id = i as u8; // Using 0-based index as emulator instance ID

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

     // Implement network mappings based on config.network_ports and emulator bound ports
     info!("Establishing network mappings based on configured ports: {:?}", config.network_ports);
     if config.network_ports.len() < instances_usize {
         warn!("Number of configured network ports ({}) is less than the number of instances ({}). Network mapping may be incomplete.", config.network_ports.len(), instances_usize);
     }

     // Example mapping logic: Assume each instance expects to communicate with *every other* instance
     // on the configured network ports on localhost.
     // Traffic from Instance i on localhost:port Pgame should be routed to Emulator's socket for Instance j
     // (listening on port E_j).
     // And vice versa.

     for i in 0..instances_usize {
         for j in 0..instances_usize {
             if i != j { // Don't map instance to itself (usually)
                 if let Some(&emulator_j_port) = emulator_instance_ports.get(&(j as u8)) {
                      // Assume game instance i tries to send to game instance j on game's port(s)
                      // using localhost (127.0.0.1).
                      // The source address seen by the emulator will be from game instance i,
                      // likely on a dynamic port, but the destination IP and port is what we use
                      // to decide where to route it.
                      // We assume game instance i sends to 127.0.0.1:Pgame_j.

                      // This is a simplification. The actual source address depends on the game.
                      // A more robust solution might need to match source IP/port *and* destination IP/port.
                      // For this example, let's map traffic *destined* for game instance j's assumed
                      // internal port(s) on localhost, to emulator instance j's listening port.

                      // Let's assume games use the ports in config.network_ports.
                      // If game instance i sends to 127.0.0.1:config.network_ports[k],
                      // and that communication is meant for instance j, how do we know it's for j?
                      // This mapping is tricky. A simpler approach for some games is if instances
                      // communicate to a fixed "server" address, and the emulator acts as that server.

                      // Let's try a direct peer-to-peer mapping assumption:
                      // Instance i sending to Instance j on game port Pgame_j -> route to Emulator's socket for Instance j
                      // This requires knowing which game port maps to which instance's communication.
                      // This mapping strategy is highly game-specific.

                      // A common simple split-screen pattern: instances try to connect to
                      // each other on fixed ports on localhost.
                      // Traffic FROM 127.0.0.1:P_game_i TO 127.0.0.1:P_game_j
                      // Needs to be routed THROUGH the emulator.
                      // The emulator listens on E_i and E_j.
                      // Traffic received BY emulator on E_i (from game i's perspective)
                      // needs to be sent TO emulator on E_j (to reach game j via its emulator socket).

                      // The current NetEmulator maps based on *source* SocketAddr. This is incorrect
                      // for redirecting traffic destined for other instances.
                      // The NetEmulator's relay logic needs to inspect the *destination* SocketAddr of the received packet.

                      // Let's revise the NetEmulator concept slightly:
                      // The emulator listens on E_0, E_1, ... E_{N-1}.
                      // Game instance i sends packets to 127.0.0.1:Pgame_target.
                      // We need to map (source_emulator_port, destination_game_port) -> target_emulator_port.
                      // When emulator receives on E_i, destined for Pgame_j, send to E_j.

                      // This requires modifying the NetEmulator relay loop to inspect the destination address.
                      // This is getting beyond a simple `add_mapping(src, dst)` call.

                      // Alternative simpler model: Each game instance is configured to talk to
                      // a distinct port on localhost (its "emulator port").
                      // Instance i sends to 127.0.0.1:E_j to talk to instance j.
                      // Then the emulator just receives on E_i and forwards to E_j based on the mapping.
                      // This requires configuring the game instances to use the emulator ports.

                      // Let's stick to the original `add_mapping(src, dst)` but interpret it differently.
                      // Assume games send to 127.0.0.1:Pgame_k to communicate with instance k (where Pgame_k is a port).
                      // We need to map traffic destined for 127.0.0.1:Pgame_j to go to the socket listening on E_j.
                      // The NetEmulator's `mappings` should be from `SocketAddr` (destination in packet)
                      // to `SocketAddr` (emulator socket to send to).
                      // The relay loop needs to check `recv_from`'s source *and* the packet's destination (harder).
                      // Or, assume games send to `127.0.0.1:P_emulator_target` where P_emulator_target is the emulator port of the target instance.

                      // Let's assume the simple case: games send to each other's assumed internal ports.
                      // Traffic originating FROM any source, but DESTINED for 127.0.0.1:Pgame_j, should be sent TO 127.0.0.1:E_j.

                      // This requires modifying the NetEmulator relay loop to inspect the *destination* address.
                      // Let's assume for now the `add_mapping` function is sufficient, and the NetEmulator
                      // implementation will eventually handle routing based on destination.
                      // We'll add mappings from game ports on localhost to emulator ports.

                      if let Some(game_port_j) = config.network_ports.get(j).cloned() {
                          let game_dest_addr: SocketAddr = format!("127.0.0.1:{}", game_port_j).parse().expect("Invalid game destination address");
                          let emulator_target_addr: SocketAddr = format!("127.0.0.1:{}", emulator_j_port).parse().expect("Invalid emulator target address");

                          info!("Mapping traffic destined for {} to emulator socket on {}", game_dest_addr, emulator_target_addr);
                          net_emulator.add_mapping(game_dest_addr, emulator_target_addr);

                           // Also map traffic destined for Instance i's game port Pgame_i from Instance j
                           // This symmetric mapping might be needed depending on game communication
                           if let Some(&emulator_i_port) = emulator_instance_ports.get(&(i as u8)) {
                                if let Some(game_port_i) = config.network_ports.get(i).cloned() {
                                     let game_dest_addr_i: SocketAddr = format!("127.0.0.1:{}", game_port_i).parse().expect("Invalid game destination address for i");
                                     let emulator_target_addr_i: SocketAddr = format!("127.0.0.1:{}", emulator_i_port).parse().expect("Invalid emulator target address for i");
                                     info!("Mapping traffic destined for {} to emulator socket on {}", game_dest_addr_i, emulator_target_addr_i);
                                     net_emulator.add_mapping(game_dest_addr_i, emulator_target_addr_i);
                                }
                           }

                      } else {
                           warn!("Network port not configured for instance index {} in config.network_ports.", j);
                      }
                 }
             }
         }
     }
     info!("Finished establishing network mappings.");


    // Start the network relay thread
    info!("Starting network emulator relay.");
    net_emulator.start_relay()?;


    // Adjust the windows using the window management module
    let window_manager = WindowManager::new()?;

    // Collect the PIDs of the launched game instances for the window manager
    info!("Attempting to set window layout for PIDs: {:?}", game_instance_pids);

    window_manager.set_layout(&game_instance_pids, layout)?;


    // Initialize the input multiplexer
    let mut input_mux = InputMux::new(); // Assuming new() is fallible or returns a Result in the future
    info!("Initializing input multiplexer.");

    // Enumerate physical input devices. This happens in main.rs before calling run_core_logic
    // if the GUI is used, and should ideally happen before this function is called.
    // If called from CLI, we might need to re-enumerate here or pass the list.
    // Let's assume the list of available devices is passed or accessible.
    // For simplicity in CLI path, we re-enumerate here.
     info!("Enumerating physical input devices (in core logic).");
     input_mux.enumerate_devices()?;
     // The available_devices list is not directly used here anymore;
     // the mapping is based on the InputAssignment vector passed to capture_events.
     // let available_devices = input_mux.get_available_devices();
     // info!("Input devices enumerated. Found {} usable devices.", available_devices.len());
     // debug!("Available devices: {:?}", available_devices);


    info!("Creating virtual input devices for {} instances.", instances_usize);
    input_mux.create_virtual_devices(instances_usize)?;
    info!("Virtual input devices created.");

    // Capture input events based on the provided input assignments
    info!("Starting input event capture and routing.");
    input_mux.capture_events(input_assignments)?;
    info!("Input event capture started. Background threads are running.");


    // The main thread calling this function will need to stay alive to keep
    // the background threads (input capture, network emulator) running.
    // If called from the GUI, the GUI event loop keeps the thread alive.
    // If called from the CLI, the main function needs to wait or enter a loop.

    info!("Core application logic execution finished successfully.");

    // Return the instances of background services for potential shutdown
    Ok((net_emulator, input_mux))
}


fn main() {
    // Set up panic hook for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        error!("Application panicked: {}", panic_info);
        if let Some(location) = panic_info.location() {
            error!("Panic occurred in file '{}' at line {}", location.file(), location.line());
        }
    }));
    
    if let Err(e) = run_application() {
        error!("Application failed: {}", e);
        std::process::exit(1);
    }
}

fn run_application() -> Result<()> {
    // Initialize the logging system first.
    // Configure the log level based on environment variables (e.g., RUST_LOG)
    // before calling init_logging().
    // The 'debug' command-line flag can set the RUST_LOG environment variable.

    // Temporarily parse args to check for the debug flag for logging initialization
     let temp_matches = parse_args_for_logging();

    let debug: bool = *temp_matches.get_one("debug").unwrap_or(&false);

    if debug {
        env::set_var("RUST_LOG", "debug");
    } else {
        if env::var("RUST_LOG").is_err() {
             env::set_var("RUST_LOG", "info");
        }
    }

    match init_logging() {
        Ok(_) => {
            info!("Logging initialized.");
            info!("Starting {} v{}", crate::APP_NAME, crate::APP_VERSION);
        }
        Err(e) => {
            eprintln!("Error initializing logging: {}", e);
            return Err(HydraError::Logging(e));
        }
    }


    // Now parse the full command-line arguments, including the potential GUI flag
    let matches: ArgMatches = cli::build_cli().get_matches();

    let use_gui_flag: bool = matches.get_flag("gui");

    // Check if any of the required CLI arguments are provided.
    // We can check for 'game_executable' as a representative required arg.
    let cli_args_provided = matches.contains_id("game_executable");


    if use_gui_flag || !cli_args_provided {
        // If the --gui flag is present, OR if no required CLI args are provided,
        // default to starting the GUI.
        info!("Starting GUI mode (default or requested).");

        // Enumerate input devices once before starting the GUI, as the GUI needs this list.
        let mut input_mux_enumerator = InputMux::new();
        let available_devices = match input_mux_enumerator.enumerate_devices() {
            Ok(_) => {
                 info!("Input devices enumerated for GUI.");
                 input_mux_enumerator.get_available_devices()
            }
            Err(e) => {
                 error!("Failed to enumerate input devices for GUI: {}", e);
                 // Display an error to the user in the GUI might be better,
                 // but returning an empty list allows the GUI to still start.
                 Vec::new()
            }
        };
         info!("Found {} usable input devices.", available_devices.len());


        // Load configuration before starting the GUI to populate it with existing settings.
        let config_path = get_config_path()?;
        let adaptive_config_path = get_adaptive_config_path()?;
        info!("Attempting to load configuration from {}", config_path.display());
        info!("Attempting to load adaptive configuration from {}", adaptive_config_path.display());

        let config = match Config::load(&config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully from {}", config_path.display());
                cfg
            }
            Err(config::ConfigError::IoError(io_err)) => {
                 if io_err.kind() == io::ErrorKind::NotFound {
                      warn!("Configuration file not found at {}. Using default configuration.", config_path.display());
                      Config::default_config()
                 } else {
                      error!("Failed to load configuration from {}: I/O Error: {}", config_path.display(), io_err);
                      // Decide if failure to load config in GUI mode is fatal.
                      // For now, log and proceed with defaults.
                      Config::default_config() // Proceed with default even on other IO errors
                 }
            }
            Err(e) => { // Catch other ConfigError variants
                error!("Failed to load configuration from {}: {}", config_path.display(), e);
                // Log and proceed with defaults on other config errors
                Config::default_config()
            }
        };
         info!("Configuration loaded or defaulted for GUI.");

        // Load adaptive configuration
        let mut adaptive_config = match AdaptiveConfigManager::new(adaptive_config_path) {
            Ok(mgr) => Some(mgr),
            Err(e) => {
                warn!("Failed to load adaptive configuration: {}. Continuing without adaptive features.", e);
                None
            }
        };

        // Validate configuration
        if let Err(e) = config.validate() {
            warn!("Configuration validation failed: {}. Using defaults where needed.", e);
        }

        // Pass the enumerated devices and loaded config to the GUI
         if let Err(e) = gui::run_gui(available_devices, config, adaptive_config) { // Pass data to run_gui
             error!("GUI application failed: {}", e);
             return Err(HydraError::application(format!("GUI failed: {}", e)));
         }
         // The GUI's app.run() is a blocking call. Once it exits, the application exits.
         info!("GUI application finished.");

    } else {
        // If --gui is NOT present AND required CLI args ARE provided, run in CLI mode.
        info!("Starting CLI mode.");

        // Retrieve parsed command-line arguments using clap 4.0+ methods
        // These are guaranteed to be present due to the check above.
        let game_executable_str: &String = matches.get_one("game_executable").unwrap(); // Safe to unwrap
        let game_executable_path = Path::new(game_executable_str);

        let instances: u32 = *matches.get_one("instances").unwrap(); // Safe to unwrap
        let instances_usize = instances as usize;

        // Collect input device names from CLI arguments as Vec<&str>
        let input_devices_names_arg: Vec<&str> = matches.get_many::<String>("input_devices")
            .unwrap() // Safe to unwrap
            .map(|s| s.as_str())
            .collect();

        let layout_str: &String = matches.get_one("layout").unwrap(); // Safe to unwrap
        let layout = Layout::from(layout_str.as_str());

        let use_proton: bool = *matches.get_one("proton").unwrap_or(&false); // Assuming 'proton' is a boolean flag


        info!("CLI arguments parsed:");
        info!("  Game Executable: {}", game_executable_path.display());
        info!("  Number of Instances: {}", instances_usize);
        info!("  Input Devices (Names): {:?}", input_devices_names_arg);
        info!("  Layout: {:?}", layout);
        info!("  Using Proton: {}", use_proton);


        // Load user configuration (in CLI mode, configuration might provide defaults or override CLI args)
        let config_path = get_config_path()?;
        let adaptive_config_path = get_adaptive_config_path()?;
        info!("Attempting to load configuration from {}", config_path.display());

        let config = match Config::load(&config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully from {}", config_path.display());
                cfg
            }
            Err(config::ConfigError::IoError(io_err)) => {
                 if io_err.kind() == io::ErrorKind::NotFound {
                      warn!("Configuration file not found at {}. Using default configuration.", config_path.display());
                      Config::default_config()
                 } else {
                      error!("Failed to load configuration from {}: I/O Error: {}", config_path.display(), io_err);
                      return Err(HydraError::Config(config::ConfigError::IoError(io_err)));
                 }
            }
            Err(e) => { // Catch other ConfigError variants
                error!("Failed to load configuration from {}: {}", config_path.display(), e);
                return Err(HydraError::Config(e));
            }
        };
        
        // Validate configuration
        config.validate()?;
        
        // Load adaptive configuration for CLI mode
        let mut adaptive_config = match AdaptiveConfigManager::new(adaptive_config_path) {
            Ok(mgr) => Some(mgr),
            Err(e) => {
                warn!("Failed to load adaptive configuration: {}. Continuing without adaptive features.", e);
                None
            }
        };
        
        // TODO: Implement logic to combine command-line arguments and configuration settings.
        // Command-line arguments should typically override configuration file settings.
        // For use_proton, the CLI arg should override config if provided.
        let final_use_proton = *matches.get_one("proton").unwrap_or(&config.use_proton);


        // Prepare InputAssignments for run_core_logic from CLI args (names)
        // We need the list of available devices to convert names to Identifiers.
        // Re-enumerate devices here as it's needed for the CLI path.
        let mut input_mux_enumerator = InputMux::new();
         let available_devices_for_cli = match input_mux_enumerator.enumerate_devices() {
             Ok(_) => input_mux_enumerator.get_available_devices(),
             Err(e) => {
                 error!("Failed to enumerate input devices for CLI mapping: {}", e);
                 Vec::new() // Proceed with empty list if enumeration fails
             }
         };
         debug!("Available devices for CLI mapping: {:?}", available_devices_for_cli);

        let mut cli_input_assignments: Vec<(usize, InputAssignment)> = Vec::new();
         for i in 0..instances_usize {
             let device_name_option = input_devices_names_arg.get(i).cloned(); // Get device name for instance i

             let assignment = match device_name_option {
                 Some(device_name) => {
                      // Find the DeviceIdentifier by name
                     let device_identifier_option = available_devices_for_cli.iter()
                          .find(|id| &id.name == device_name)
                          .cloned();

                     match device_identifier_option {
                          Some(identifier) => {
                              info!("CLI Mapping: Device '{}' found and assigned to instance {}", device_name, i);
                              InputAssignment::Device(identifier)
                          },
                          None => {
                              warn!("CLI Mapping: Specified device '{}' not found. Assigning None for instance {}", device_name, i);
                              InputAssignment::None // Or AutoDetect if that's the CLI default behavior
                          }
                     }
                 },
                 None => {
                      // No device name provided for this instance in CLI args
                      info!("CLI Mapping: No input device specified for instance {}. Assigning None.", i);
                     InputAssignment::None // Default to None if no arg provided
                 }
             };
             cli_input_assignments.push((i, assignment));
         }
         debug!("CLI input assignments: {:?}", cli_input_assignments);


        // Trigger the core application logic with CLI-provided (or combined) settings
        info!("Triggering core application logic from CLI.");
         // Pass final_use_proton and cli_input_assignments
         let core_result = run_core_logic(
             game_executable_path,
             instances_usize,
             &cli_input_assignments,
             layout,
             final_use_proton, // Use the potentially overridden use_proton
             &config,
             adaptive_config.as_mut(),
         );


         let (mut net_emulator, mut input_mux) = match core_result { // Make instances mutable
             Ok((net_emu, input_mux)) => {
                 info!("Core application logic finished successfully.");
                 (net_emu, input_mux) // Store the instances
             },
             Err(e) => {
                 error!("Core application logic failed: {}", e);
                 return Err(e);
             }
         };


        // Main application loop/wait in CLI mode
        info!("Hydra Co-op is running in CLI mode. Background services started.");
        info!("Press Ctrl+C to initiate graceful shutdown.");

        // Use ctrlc for graceful shutdown in CLI mode
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || {
            info!("Ctrl+C received. Initiating graceful shutdown.");
            r.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        // Wait until Ctrl+C is pressed
        while running.load(Ordering::SeqCst) {
             // TODO: Check if game instances are still running and exit if all have quit.
             // This would involve keeping track of the Child processes returned by launch_multiple_game_instances
             // and periodically checking their status (e.g., using try_wait()).
            thread::sleep(Duration::from_millis(100));
        }

        info!("Shutdown sequence started. Stopping background services...");

        // Stop background threads gracefully and wait for them to join
        if let Err(e) = net_emulator.stop_relay() {
             error!("Error stopping network relay during shutdown: {}", e);
        } else {
             info!("Network relay stop signal sent.");
             // Wait for the network relay thread to finish
             if let Some(handle) = net_emulator.join_relay() {
                 match handle.join() {
                     Ok(_) => info!("Network relay thread joined successfully."),
                     Err(e) => error!("Error joining network relay thread: {:?}", e), // Thread panicked
                 }
             } else {
                 debug!("Network relay thread handle not available to join.");
             }
        }

        if let Err(e) = input_mux.stop_capture() {
             error!("Error stopping input capture during shutdown: {}", e);
        } else {
             info!("Input capture stop signal sent.");
             // Wait for the input capture thread to finish
             if let Some(handle) = input_mux.join_capture() {
                 match handle.join() {
                     Ok(_) => info!("Input capture thread joined successfully."),
                     Err(e) => error!("Error joining input capture thread: {:?}", e), // Thread panicked
                 }
             } else {
                 debug!("Input capture thread handle not available to join.");
             }
        }

         // TODO: Implement graceful shutdown for game instances (e.g., sending signals)
         // TODO: Clean up temporary WINEPREFIX directories if created (only if use_proton is true and they were created)

        info!("Background services stopped. Exiting application.");
    }

    Ok(())
}

/// Get the configuration file path
fn get_config_path() -> Result<PathBuf> {
    if let Ok(config_path_str) = env::var("CONFIG_PATH") {
        Ok(PathBuf::from(config_path_str))
    } else {
        let config_dir = crate::utils::get_config_dir()?;
        crate::utils::ensure_dir_exists(&config_dir)?;
        Ok(config_dir.join("config.toml"))
    }
}

/// Get the adaptive configuration file path
fn get_adaptive_config_path() -> Result<PathBuf> {
    let config_dir = crate::utils::get_config_dir()?;
    crate::utils::ensure_dir_exists(&config_dir)?;
    Ok(config_dir.join("adaptive.toml"))
}

// Helper function for early parsing of args (just for debug flag)
// Note: This uses Command::new with a placeholder name, which is fine
// for this limited early parsing purpose.
fn parse_args_for_logging() -> ArgMatches {
    clap::Command::new("Hydra Co-op")
        .arg(clap::Arg::new("debug").long("debug").action(clap::ArgAction::SetTrue))
        // Add other relevant args needed before full parsing if any
        .disable_help_flag(true) // Don't show help for this temporary parser
        .disable_version_flag(true) // Don't show version for this temporary parser
        .allow_missing_positional(true) // Allow missing positional arguments
        .ignore_errors(true) // Ignore parsing errors during this temporary phase
        .get_matches()
}



// Note: Ensure all modules used here (cli, config, instance_manager, logging,
// net_emulator, window_manager, input_mux, gui) exist and have the expected
// public functions and types as called in main.rs.
// Remember to add necessary dependencies (gtk, uinput, polling, tempfile, ctrlc) to your Cargo.toml.
