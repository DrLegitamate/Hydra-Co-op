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
// | Decide CLI or GUI Mode      | // New step
// +-----------------------------+
//              /   \
//             v     v
//      +------------+ +------------+
//      | CLI Logic  | | GUI Logic  |
//      +------------+ +------------+
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


use crate::cli::parse_args;
use crate::config::{Config, ConfigError}; // Import ConfigError
use crate::instance_manager::{launch_multiple_game_instances, InstanceManagerError}; // Import InstanceManagerError
use crate::logging::init as init_logging; // Alias to avoid name conflict if another 'init' exists
use crate::net_emulator::{NetEmulator, NetEmulatorError}; // Import NetEmulatorError
use crate::window_manager::{WindowManager, Layout, WindowManagerError}; // Import WindowManagerError
use crate::input_mux::{InputMux, InputMuxError, DeviceIdentifier}; // Import InputMuxError and DeviceIdentifier
use std::{env, thread};
use log::{info, error, warn, debug}; // Import warn and debug for consistency
use std::path::{Path, PathBuf}; // Import Path and PathBuf
use clap::{ArgMatches, Arg}; // Import Arg for adding GUI flag
use std::time::Duration;
use std::collections::HashMap; // Import HashMap
use std::process::Child; // Import Child if needed for instance management
use std::fs; // Import fs for creating WINEPREFIX base directory
use log::SetLoggerError; // Import SetLoggerError

// Assuming your GUI code is in src/gui.rs and has a public run_gui function
mod gui; // Declare the gui module

fn main() {
    // Initialize the logging system first.
    // Configure the log level based on environment variables (e.g., RUST_LOG)
    // before calling init_logging().
    // The 'debug' command-line flag can set the RUST_LOG environment variable.

    // Temporarily parse args to check for the debug flag for logging initialization
    // A more robust approach would be to have a dedicated logging setup function
    // that can be called before full argument parsing.
     let temp_matches = parse_args_for_logging(); // Custom function for early parsing

    let debug: bool = *temp_matches.get_one("debug").unwrap_or(&false); // Get the debug flag

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
    let matches: ArgMatches = build_cli_with_gui_flag().get_matches(); // Use a modified build_cli

    let use_gui: bool = *matches.get_one("gui").unwrap_or(&false); // Check for the --gui flag


    if use_gui {
        info!("Starting GUI mode.");
        // The GUI will handle configuration and triggering the core logic.
         if let Err(e) = gui::run_gui() { // Call the public function from gui.rs
             error!("GUI application failed: {}", e);
             std::process::exit(1);
         }
         // The GUI's app.run() is a blocking call. Once it exits, the application exits.
         info!("GUI application finished.");

    } else {
        info!("Starting CLI mode.");
        // In CLI mode, we proceed with parsing arguments and executing core logic directly.

        // Retrieve parsed command-line arguments using clap 4.0+ methods
        let game_executable_str: &String = matches.get_one("game_executable").expect("game_executable argument missing in CLI mode");
        let game_executable_path = Path::new(game_executable_str);

        let instances: u32 = *matches.get_one("instances").expect("instances argument missing in CLI mode");
        let instances_usize = instances as usize;

        let input_devices_arg: Vec<&str> = matches.get_many::<String>("input_devices")
            .expect("input_devices argument missing in CLI mode")
            .map(|s| s.as_str())
            .collect();

        let layout_str: &String = matches.get_one("layout").expect("layout argument missing in CLI mode");
        let layout = Layout::from(layout_str.as_str());

        let use_proton: bool = *matches.get_one("proton").unwrap_or(&false); // Assuming 'proton' is a boolean flag


        info!("CLI arguments parsed:");
        info!("  Game Executable: {}", game_executable_path.display());
        info!("  Number of Instances: {}", instances_usize);
        info!("  Input Devices: {:?}", input_devices_arg);
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
                      std::process::exit(1);
                 }
            }
            Err(e) => { // Catch other ConfigError variants
                error!("Failed to load configuration from {}: {}", config_path.display(), e);
                std::process::exit(1);
            }
        };
        // TODO: Implement logic to combine command-line arguments and configuration settings.
        // Command-line arguments should typically override configuration file settings.


        // Determine the base directory for WINEPREFIXes if using Proton (CLI mode).
        let base_wineprefix_dir = if use_proton {
            let mut dir = env::temp_dir();
            dir.push("hydra_coop_wineprefixes");
            info!("Using base directory for WINEPREFIXes: {}", dir.display());
             if let Err(e) = fs::create_dir_all(&dir) {
                  error!("Failed to create base WINEPREFIX directory {}: {}", dir.display(), e);
                  std::process::exit(1);
             }
             dir
        } else {
            PathBuf::from("/dev/null")
        };


        // Trigger the core application logic with CLI-provided (or combined) settings

        // Launch the required number of game instances
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
                std::process::exit(1);
            }
        };

        // Note: Window management and input multiplexing should happen AFTER game instances are launched.

        // Set up the virtual network emulator to connect these instances
        let mut net_emulator = NetEmulator::new();
        info!("Initializing network emulator.");
        let mut emulator_instance_ports: HashMap<u8, u16> = HashMap::new();

        info!("Adding {} instances to network emulator.", game_instances.len());
        for (i, instance) in game_instances.iter().enumerate() {
            let emulator_instance_id = i as u8;
            let pid = instance.id();

             if emulator_instance_id as u32 != i as u32 {
                  error!("Instance index {} exceeds u8 capacity for emulator ID. Cannot add to network emulator.", i);
                  continue;
             }

            match net_emulator.add_instance(emulator_instance_id) {
                Ok(bound_port) => {
                    emulator_instance_ports.insert(emulator_instance_id, bound_port);
                    info!("Instance {} (PID: {}) added to net emulator, bound to port {}", emulator_instance_id, pid, bound_port);
                }
                Err(e) => {
                     error!("Failed to add instance {} (PID: {}) to net emulator: {}", emulator_instance_id, pid, e);
                }
            }
        }

        // TODO: Establish network mappings (src -> dst SocketAddr) based on config and game needs.


        info!("Starting network emulator relay.");
        if let Err(e) = net_emulator.start_relay() {
             error!("Failed to start network emulator relay: {}", e);
             std::process::exit(1);
        }
        info!("Network emulator relay started in background thread.");


        // Adjust the windows using the window management module
        let window_manager = match WindowManager::new() {
            Ok(wm) => {
                info!("Window manager initialized successfully.");
                wm
            }
            Err(e) => {
                error!("Failed to initialize window manager: {}", e);
                std::process::exit(1);
            }
        };

        let game_instance_pids: Vec<u32> = game_instances.iter().map(|instance| instance.id()).collect();
        info!("Attempting to set window layout for PIDs: {:?}", game_instance_pids);

        if let Err(e) = window_manager.set_layout(&game_instance_pids, layout) {
             match e {
                 WindowManagerError::WindowNotFound => {
                     error!("Failed to set window layout: One or more game windows were not found after launch.");
                 }
                 WindowManagerError::MonitorDetectionError(msg) => {
                      error!("Failed to set window layout: Monitor detection error: {}", msg);
                 }
                 other_error => {
                      error!("Failed to set window layout: An unexpected error occurred: {}", other_error);
                 }
             }
             std::process::exit(1);
        }
        info!("Window layout set successfully.");


        // Initialize the input multiplexer
        let mut input_mux = InputMux::new();
        info!("Initializing input multiplexer.");

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
            std::process::exit(1);
        }
        let available_devices = input_mux.get_available_devices();
        info!("Input devices enumerated. Found {} usable devices.", available_devices.len());
        debug!("Available devices: {:?}", available_devices);

        info!("Creating virtual input devices for {} instances.", instances_usize);
        if let Err(e) = input_mux.create_virtual_devices(instances_usize) {
            match e {
                InputMuxError::UinputError(uinput_e) => {
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
            std::process::exit(1);
        }
        info!("Virtual input devices created.");

        // Map input devices to instances based on command-line arguments (or combined config/args)
        if input_devices_arg.is_empty() {
             warn!("No input devices specified via command line ('-d' argument). Input multiplexing may not work as intended.");
        } else {
             info!("Mapping specified input devices to instances.");

             if input_devices_arg.len() > instances_usize {
                  warn!("More input devices specified ({}) than launched instances ({}). Extra devices will not be mapped.", input_devices_arg.len(), instances_usize);
             }

             for (instance_index, device_name_arg) in input_devices_arg.iter().enumerate() {
                  if instance_index >= instances_usize {
                      info!("Skipping mapping for device '{}' as instance index {} is out of bounds.", device_name_arg, instance_index);
                      break;
                  }

                  let device_identifier_option = available_devices.iter()
                       .find(|id| id.name == *device_name_arg)
                       .cloned();

                  match device_identifier_option {
                       Some(identifier) => {
                            if let Err(e) = input_mux.map_device_to_instance_by_identifier(identifier, instance_index) {
                                 error!("Failed to map device '{}' to instance {}: {}", device_name_arg, instance_index, e);
                            } else {
                                 info!("Successfully mapped device '{}' to instance {}.", device_name_arg, instance_index);
                            }
                       }
                       None => {
                            warn!("Specified input device '{}' not found among available devices. Cannot map to instance {}. Available devices: {:?}", device_name_arg, instance_index, available_devices);
                       }
                  }
             }
        }

        // Capture input events. This will spawn threads and keep the application running.
        info!("Starting input event capture and routing.");
        if let Err(e) = input_mux.capture_events() {
             error!("Error during input event capture setup: {}", e);
             std::process::exit(1);
        }
        info!("Input event capture started. Background threads are running.");

        // Main application loop/wait in CLI mode
        info!("Hydra Co-op is running in CLI mode. Background services started.");
        info!("Press Ctrl+C to initiate shutdown.");

        // Simple blocking loop to keep the main thread alive in CLI mode
         loop {
             thread::sleep(Duration::from_secs(60));
         }
    }

}

// Helper function for early parsing of args (just for debug flag)
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
    // Get the base CLI definition
    let mut cli = crate::cli::build_cli();
    // Add the --gui flag
    cli = cli.arg(
        Arg::new("gui")
            .long("gui")
            .help("Launches the application with the graphical user interface")
            .action(clap::ArgAction::SetTrue)
            // Make the GUI flag conflict with required CLI arguments
            // If --gui is present, required CLI args are not needed.
             .conflicts_with_all(&["game_executable", "instances", "input_devices", "layout"])
    );
    cli
}


// Note: Ensure all modules used here (cli, config, instance_manager, logging,
// net_emulator, window_manager, input_mux, gui) exist and have the expected
// public functions and types as called in main.rs.
// Remember to add necessary dependencies (gtk, uinput, polling, tempfile, ctrlc) to your Cargo.toml.
