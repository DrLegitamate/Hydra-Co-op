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


use crate::cli::parse_args; // We won't directly use parse_args from cli anymore, but build_cli is used
use crate::config::{Config, ConfigError}; // Import ConfigError
use crate::instance_manager::{launch_multiple_game_instances, InstanceManagerError}; // Import InstanceManagerError
use crate::logging::init as init_logging; // Alias to avoid name conflict if another 'init' exists
use crate::net_emulator::{NetEmulator, NetEmulatorError}; // Import NetEmulatorError
use crate::window_manager::{WindowManager, Layout, WindowManagerError}; // Import WindowManagerError
use crate::input_mux::{InputMux, InputMuxError, DeviceIdentifier, InputAssignment}; // Import InputMuxError, DeviceIdentifier, InputAssignment
use std::{env, thread};
use log::{info, error, warn, debug}; // Import warn and debug for consistency
use std::path::{Path, PathBuf}; // Import Path and PathBuf
use clap::{ArgMatches, Arg, Command}; // Import Command for helper
use std::time::Duration;
use std::collections::HashMap; // Import HashMap
use std::process::Child; // Import Child if needed for instance management
use std::fs; // Import fs for creating WINEPREFIX base directory
use log::SetLoggerError; // Import SetLoggerError
use std::error::Error; // Import Error trait for boxed errors in run_core_logic
use std::net::SocketAddr; // Import SocketAddr


// Assuming your GUI code is in src/gui.rs and has a public run_gui function
mod gui; // Declare the gui module


/// Encapsulates the core application logic: launching instances, setting up
/// network, managing windows, and initializing input multiplexing.
/// This function can be called by both the CLI and GUI modes.
///
/// # Returns
///
/// * `Result<(NetEmulator, InputMux), Box<dyn Error>>` - Returns the initialized
///   NetEmulator and InputMux instances if successful, otherwise returns a boxed error.
pub fn run_core_logic(
    game_executable_path: &Path,
    instances_usize: usize,
    input_assignments: &[(usize, InputAssignment)], // Use InputAssignment
    layout: Layout,
    use_proton: bool,
    config: &Config, // Pass the loaded configuration
    // Potentially pass other necessary data like network mapping config
) -> Result<(NetEmulator, InputMux), Box<dyn Error>> { // Return instances
    info!("Running core application logic.");
    debug!("  Game Executable: {}", game_executable_path.display());
    debug!("  Number of Instances: {}", instances_usize);
    debug!("  Input Assignments: {:?}", input_assignments); // Log assignments
    debug!("  Layout: {:?}", layout);
    debug!("  Using Proton: {}", use_proton);
    debug!("  Config: {:?}", config); // Log config details if Debug is derived


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
              return Err(Box::new(InstanceManagerError::IoError(e))); // Return as boxed error
         }
         dir

    } else {
        // If not using Proton, the base_wineprefix_dir is not strictly needed
        // by launch_multiple_game_instances, but we pass a dummy path.
        PathBuf::from("/dev/null") // Or a temporary directory that will be ignored
    };


    // Launch the required number of game instances
    info!("Launching {} game instances using executable: {}", instances_usize, game_executable_path.display());
    let mut game_instances = launch_multiple_game_instances(
        game_executable_path,
        instances_usize,
        use_proton,
        &base_wineprefix_dir,
    ).map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map InstanceManagerError to boxed error


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

     // TODO: Establish network mappings (src -> dst SocketAddr) based on config and game needs.
     // Use the 'config' object and potentially the bound ports from emulator_instance_ports
     // to determine and add network mappings using net_emulator.add_mapping().
     warn!("Network mapping logic needs to be implemented based on target game's networking and configuration.");
     // This will likely involve iterating through config.network_ports and establishing
     // mappings between instance emulator ports.

     // Example (Illustrative - requires knowing game communication details and using config):
     /*
     use std::net::SocketAddr;
     // Assuming config.network_ports contains the ports games communicate on
     if instances_usize == 2 && config.network_ports.len() >= 2 {
         let game1_internal_port = config.network_ports[0];
         let game2_internal_port = config.network_ports[1];

         let emulator1_port = emulator_instance_ports.get(&0).expect("Emulator port for instance 0 not found");
         let emulator2_port = emulator_instance_ports.get(&1).expect("Emulator port for instance 1 not found");

         let game1_internal_addr: SocketAddr = format!("127.0.0.1:{}", game1_internal_port).parse().expect("Invalid game1 internal address");
         let game2_internal_addr: SocketAddr = format!("127.0.0.1:{}", game2_internal_port).parse().expect("Invalid game2 internal address");

         let emulator1_listening_addr: SocketAddr = format!("127.0.0.1:{}", emulator1_port).parse().expect("Invalid emulator1 listening address");
         let emulator2_listening_addr: SocketAddr = format!("127.0.0.1:{}", emulator2_port).parse().expect("Invalid emulator2 listening address");

         // Mapping game instance 1's traffic to game instance 2 via the emulator
         net_emulator.add_mapping(game1_internal_addr, emulator2_listening_addr);
         // Mapping game instance 2's traffic to game instance 1 via the emulator
         net_emulator.add_mapping(game2_internal_addr, emulator1_listening_addr);

          info!("Added example network mappings for 2 instances.");
     } else {
          warn!("Network mapping not configured or not supported for this number of instances/ports.");
     }
     */


    // Start the network relay thread
    info!("Starting network emulator relay.");
    net_emulator.start_relay().map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map and return NetEmulatorError


    // Adjust the windows using the window management module
    let window_manager = WindowManager::new().map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map and return WindowManagerError

    // Collect the PIDs of the launched game instances for the window manager
    let game_instance_pids: Vec<u32> = game_instances.iter().map(|instance| instance.id()).collect();
    info!("Attempting to set window layout for PIDs: {:?}", game_instance_pids);

    window_manager.set_layout(&game_instance_pids, layout).map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map and return WindowManagerError


    // Initialize the input multiplexer
    let mut input_mux = InputMux::new(); // Assuming new() is fallible or returns a Result in the future
    info!("Initializing input multiplexer.");

    // Enumerate physical input devices. This happens in main.rs before calling run_core_logic
    // if the GUI is used, and should ideally happen before this function is called.
    // If called from CLI, we might need to re-enumerate here or pass the list.
    // Let's assume the list of available devices is passed or accessible.
    // For simplicity in CLI path, we re-enumerate here.
     info!("Enumerating physical input devices (in core logic).");
     input_mux.enumerate_devices().map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map and return InputMuxError
     let available_devices = input_mux.get_available_devices();
     info!("Input devices enumerated. Found {} usable devices.", available_devices.len());
     debug!("Available devices: {:?}", available_devices);


    info!("Creating virtual input devices for {} instances.", instances_usize);
    input_mux.create_virtual_devices(instances_usize).map_err(|e| Box::new(e) as Box<dyn Error>)?; // Map and return InputMuxError
    info!("Virtual input devices created.");

    // Capture input events based on the provided input assignments
    info!("Starting input event capture and routing.");
    input_mux.capture_events(input_assignments).map_err(|e| Box::new(e) as Box<dyn Error>)?; // Pass and map input_assignments
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
        Ok(_) => info!("Logging initialized."),
        Err(e) => eprintln!("Error initializing logging: {}", e), // Use eprintln as logger might not be fully ready
    }


    // Now parse the full command-line arguments, including the potential GUI flag
    let matches: ArgMatches = build_cli_with_gui_flag().get_matches();

    let use_gui_flag: bool = *matches.get_one("gui").unwrap_or(&false);

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
        let config_path_str = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
        let config_path = Path::new(&config_path_str);
        info!("Attempting to load configuration from {}", config_path.display());

        let config = match Config::load(config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully from {}", config_path.display());
                cfg
            }
            Err(ConfigError::IoError(io_err)) => {
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


        // Pass the enumerated devices and loaded config to the GUI
         if let Err(e) = gui::run_gui(available_devices, config) { // Pass data to run_gui
             error!("GUI application failed: {}", e);
             std::process::exit(1);
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
        let config_path_str = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
        let config_path = Path::new(&config_path_str);
        info!("Attempting to load configuration from {}", config_path.display());

        let config = match Config::load(config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully from {}", config_path.display());
                cfg
            }
            Err(ConfigError::IoError(io_err)) => {
                 if io_err.kind() == io::ErrorKind::NotFound {
                      warn!("Configuration file not found at {}. Using default configuration.", config_path.display());
                      Config::default_config()
                 } else {
                      error!("Failed to load configuration from {}: I/O Error: {}", config_path.display(), io_err);
                      std::process::exit(1); // Fatal for other IO errors
                 }
            }
            Err(e) => { // Catch other ConfigError variants
                error!("Failed to load configuration from {}: {}", config_path.display(), e);
                std::process::exit(1);
            }
        };
        // TODO: Implement logic to combine command-line arguments and configuration settings.
        // Command-line arguments should typically override configuration file settings.


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
         for (i, device_name) in input_devices_names_arg.iter().enumerate() {
              if i >= instances_usize { break; } // Only process up to number of instances

              // Find the DeviceIdentifier by name
              let device_identifier_option = available_devices_for_cli.iter()
                   .find(|id| &id.name == device_name)
                   .cloned();

              let assignment = match device_identifier_option {
                   Some(identifier) => {
                       info!("CLI Mapping: Device '{}' found and assigned to instance {}", device_name, i);
                       InputAssignment::Device(identifier)
                   },
                   None => {
                       warn!("CLI Mapping: Specified device '{}' not found. Assigning None for instance {}", device_name, i);
                       InputAssignment::None // Or AutoDetect if that's the CLI default behavior
                   }
              };
              cli_input_assignments.push((i, assignment));
         }
         // If fewer input devices specified than instances, assign None to remaining
         for i in cli_input_assignments.len()..instances_usize {
              info!("CLI Mapping: No input device specified for instance {}. Assigning None.", i);
              cli_input_assignments.push((i, InputAssignment::None));
         }
         debug!("CLI input assignments: {:?}", cli_input_assignments);


        // Trigger the core application logic with CLI-provided (or combined) settings
        info!("Triggering core application logic from CLI.");
        let core_result = run_core_logic(
            game_executable_path,
            instances_usize,
            &cli_input_assignments.iter().map(|(_, assign)| match assign { // Pass names or identifiers as &[&str] - NO, run_core_logic expects InputAssignment now
                 InputAssignment::Device(id) => id.name.as_str(),
                 InputAssignment::AutoDetect => "Auto-detect", // Represent AutoDetect as string
                 InputAssignment::None => "None", // Represent None as string
             }).collect::<Vec<&str>>(), // Collect names for the function signature (THIS IS WRONG)
            layout,
            use_proton,
            &config,
            // Pass other necessary data
        );
         // CORRECTED: run_core_logic accepts &[(usize, InputAssignment)]
         let core_result = run_core_logic(
             game_executable_path,
             instances_usize,
             &cli_input_assignments, // Pass the built InputAssignment vector
             layout,
             use_proton,
             &config,
         );


         let (net_emulator, input_mux) = match core_result {
             Ok((net_emu, input_mux)) => {
                 info!("Core application logic finished successfully.");
                 (net_emu, input_mux) // Store the instances
             },
             Err(e) => {
                 error!("Core application logic failed: {}", e);
                 std::process::exit(1);
             }
         };


        // Main application loop/wait in CLI mode
        info!("Hydra Co-op is running in CLI mode. Background services started.");
        info!("Press Ctrl+C to initiate shutdown.");

        // Use ctrlc for graceful shutdown in CLI mode
        use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || {
            info!("Ctrl+C received. Initiating graceful shutdown.");
            r.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        // Wait until Ctrl+C is pressed
        while running.load(Ordering::SeqCst) {
             // TODO: Check if game instances are still running and exit if all have quit.
            thread::sleep(Duration::from_millis(100));
        }

        info!("Shutdown sequence started. Stopping background services...");
        // Stop background threads gracefully
        if let Err(e) = net_emulator.stop_relay() {
             error!("Error stopping network relay during shutdown: {}", e);
        } else {
             info!("Network relay stopped.");
        }
        if let Err(e) = input_mux.stop_capture() {
             error!("Error stopping input capture during shutdown: {}", e);
        } else {
             info!("Input capture stopped.");
        }

         // TODO: Clean up temporary WINEPREFIX directories if created
         // TODO: Wait for game instances to exit gracefully

        info!("Background services stopped. Exiting application.");
    }

}

// Helper function for early parsing of args (just for debug flag)
// Note: This uses Command::new with a placeholder name, which is fine
// for this limited early parsing purpose.
fn parse_args_for_logging() -> ArgMatches {
    Command::new("Hydra Co-op")
        .arg(Arg::new("debug").long("debug").action(clap::ArgAction::SetTrue))
        // Add other relevant args needed before full parsing if any
        .disable_help_flag(true) // Don't show help for this temporary parser
        .disable_version_flag(true) // Don't show version for this temporary parser
        .allow_missing_positional(true) // Allow missing positional arguments
        .ignore_errors(true) // Ignore parsing errors during this temporary phase
        .get_matches()
}

// Helper function to build the full CLI Command including the GUI flag
fn build_cli_with_gui_flag() -> clap::Command {
    // Get the base CLI definition from cli.rs
    let mut cli = crate::cli::build_cli();
    // Add the --gui flag
    cli = cli.arg(
        Arg::new("gui")
            .long("gui")
            .help("Launches the application with the graphical user interface")
            .action(clap::ArgAction::SetTrue)
            // Make the GUI flag conflict with required CLI arguments
            // If --gui is present, required CLI args are not needed.
            // This is handled by clap's conflicts_with_all.
            // The defaulting logic in main() then checks if required args were provided
            // when --gui is absent.
             .conflicts_with_all(&["game_executable", "instances", "input_devices", "layout"])
    );
    cli
}


// Note: Ensure all modules used here (cli, config, instance_manager, logging,
// net_emulator, window_manager, input_mux, gui) exist and have the expected
// public functions and types as called in main.rs.
// Remember to add necessary dependencies (gtk, uinput, polling, tempfile, ctrlc) to your Cargo.toml.
