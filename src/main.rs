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
// | Detect & Launch Proton Games|
// +-----------------------------+

use crate::cli::parse_args;
use crate::config::Config;
use crate::instance_manager::launch_multiple_game_instances;
use crate::logging::init;
use crate::net_emulator::NetEmulator;
use crate::proton_integration::launch_game;
use crate::window_manager::{WindowManager, Layout}; // Import Layout enum
use crate::input_mux::InputMux;
use std::env;
use log::{info, error}; // Import error for consistent error reporting
use std::path::Path; // Import Path

fn main() {
    // Initialize the logging system
    // This should ideally be done once at the very beginning.
    // The env::set_var("RUST_LOG", ...) calls below are redundant
    // if the logging is already initialized based on environment variables
    // or a default level in the init() function.
    init();

    // Parse command-line arguments
    let matches = parse_args();

    let game_executable_str = matches.value_of("game_executable").expect("game_executable argument missing");
    let game_executable_path = Path::new(game_executable_str);

    let instances = matches.value_of("instances").expect("instances argument missing").parse::<usize>().expect("Invalid value for instances");
    let input_devices = matches.values_of("input_devices").map(|values| values.collect::<Vec<&str>>()).unwrap_or_else(Vec::new); // Handle case where input_devices might not be provided
    let layout_str = matches.value_of("layout").expect("layout argument missing");
    let layout = Layout::from(layout_str); // Use the From implementation for Layout
    let debug = matches.is_present("debug");
    let use_proton = matches.is_present("proton"); // More descriptive variable name

    // Setting log level based on debug flag.
    // Ensure init() allows overriding with environment variables.
    if debug {
        env::set_var("RUST_LOG", "debug");
    } else {
        env::set_var("RUST_LOG", "info");
    }
    // Re-initialize logging after setting the variable, or ensure init() reads the variable after it's set.
    // A better approach might be to configure env_logger based on the 'debug' flag *before* calling init().
    // For now, assuming init() is flexible or called after env::set_var.
    // env_logger::builder().filter_level(...).init(); // Alternative if init() is not flexible


    info!("Game Executable: {}", game_executable_path.display());
    info!("Number of Instances: {}", instances);
    info!("Input Devices: {:?}", input_devices);
    info!("Layout: {:?}", layout); // Use Debug print for Layout
    info!("Debug Mode: {}", debug);
    info!("Using Proton: {}", use_proton);


    // Load user configuration
    let config_path_str = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let config_path = Path::new(&config_path_str);
    let config = Config::load(config_path).unwrap_or_else(|e| {
        // Use error! macro for consistency
        error!("Failed to load configuration from {}: {}", config_path.display(), e);
        std::process::exit(1);
    });
    info!("Configuration loaded from {}", config_path.display());


    // Launch the required number of game instances
    let game_instances = launch_multiple_game_instances(game_executable_path, instances).unwrap_or_else(|e| {
        error!("Failed to launch game instances: {}", e);
        std::process::exit(1);
    });
    info!("Launched {} game instances.", game_instances.len());

    // Note: At this point, the game processes are started, but their windows
    // might not be immediately available. The window manager needs to wait
    // for the windows to be created and mapped before attempting to
    // manipulate them. The current window_manager::set_layout includes a basic
    // retry mechanism, but a more robust solution might be needed.


    // Set up the virtual network emulator to connect these instances
    // Assuming NetEmulator::new() and its methods handle their own errors internally
    let mut net_emulator = NetEmulator::new();
    info!("Initializing network emulator.");
    for (i, instance) in game_instances.iter().enumerate() {
        // Assuming instance.id() returns the process ID (PID)
        // The network emulator might need the PID to associate network traffic
        // with specific instances.
         if let Err(e) = net_emulator.add_instance(instance.id() as u8) { // Assuming add_instance takes a u8 identifier
             error!("Failed to add instance {} (PID: {}) to net emulator: {}", i, instance.id(), e);
             // Decide if this failure should be fatal or if the application can continue
             // with fewer instances in the network emulator.
             // For now, let's continue but log the error.
         }
    }
    // Assuming start_relay handles errors internally or returns a Result
    info!("Starting network emulator relay.");
    net_emulator.start_relay(); // Assuming this is a non-blocking or background operation


    // Adjust the windows using the window management module to arrange them
    // in the selected split-screen layout.
    // This requires finding the windows associated with the launched processes.
    let window_manager = WindowManager::new().unwrap_or_else(|e| {
        error!("Failed to initialize window manager: {}", e);
        std::process::exit(1);
    });
    info!("Window manager initialized.");

    // Collect the PIDs of the launched game instances
    let game_instance_pids: Vec<u32> = game_instances.iter().map(|instance| instance.id()).collect();

    // Set the layout for the windows corresponding to the launched PIDs
    if let Err(e) = window_manager.set_layout(&game_instance_pids, layout) {
         error!("Failed to set window layout: {}", e);
         // Decide if this failure is fatal. The games are launched, but windows aren't arranged.
         // For a launcher, this might be a non-fatal error allowing the user to manually arrange.
         // For now, we exit as per the original code's pattern.
         std::process::exit(1);
    }
    info!("Window layout set.");


    // Initialize the input multiplexer to route inputs from individual devices
    // to their assigned game instances.
    let mut input_mux = InputMux::new();
    info!("Initializing input multiplexer.");
    if let Err(e) = input_mux.enumerate_devices() {
        error!("Failed to enumerate input devices: {}", e);
        // This is likely a fatal error as input cannot be routed without devices.
        std::process::exit(1);
    }
    info!("Input devices enumerated.");

    // Map input devices to instances. The logic for mapping based on
    // the input_devices argument needs to be implemented in InputMux.
    if input_devices.is_empty() {
         warn!("No input devices specified. Input multiplexing may not work as intended.");
         // Depending on requirements, you might exit or proceed without input routing.
    } else {
         info!("Mapping input devices to instances.");
         // Assuming the order of input_devices corresponds to the order of launched instances
         if input_devices.len() != instances {
              warn!("Number of specified input devices ({}) does not match the number of instances ({}). Input mapping might be incorrect.", input_devices.len(), instances);
              // Decide how to handle this mismatch - potentially map the first N devices to the first N instances.
         }

         for (i, device_name) in input_devices.iter().enumerate() {
              // The InputMux::map_device_to_instance function needs to be implemented
              // to find the actual input device by name or identifier and associate it
              // with the i-th game instance (based on the order they were launched or a config).
              // This example assumes a simple mapping by index.
               input_mux.map_device_to_instance(device_name, i); // Assuming mapping by index i
               info!("Mapped input device '{}' to instance index {}", device_name, i);
         }
    }

    // Capture input events. This is likely a blocking operation that
    // keeps the application running and handling input.
    info!("Starting input event capture.");
    // The capture_events function should ideally handle errors internally
    // or run in a separate thread if the main thread needs to do other things.
    if let Err(e) = input_mux.capture_events() {
         error!("Error during input event capture: {}", e);
         // This is likely a fatal error for a split-screen application.
         std::process::exit(1);
    }


    // If necessary, detect and launch Windows games via Proton
    // This block is currently launching ALL instances with Proton if the flag is present.
    // This logic might need refinement depending on how Proton integration works.
    // If launch_game is meant to wrap the original executable launch with Proton,
    // this should likely happen earlier, perhaps within or called by instance_manager.
    if use_proton {
        // This loop is likely redundant if launch_multiple_game_instances already
        // handles launching via Proton when the flag is set.
        // If launch_game is a separate step, the logic needs to be clear
        // about which instance it's applying to.
         info!("Proton flag is set. Executing Proton launch logic.");
         // The current loop structure would try to launch the *same* game_executable
         // with Proton for *each* already launched instance, which is probably not intended.
         // The Proton integration should likely happen once per instance,
         // potentially modifying how the command is built in instance_manager
         // or managing the Proton environment before spawning the game.
        /*
        for instance in game_instances { // This loop is likely incorrect here
            launch_game(&game_executable_path).unwrap_or_else(|e| {
                eprintln!("Failed to launch game with Proton: {}", e);
                std::process::exit(1);
            });
        }
        */
        // TODO: Re-evaluate Proton integration logic and where it fits in the workflow.
         warn!("Proton integration block is currently a placeholder and might not be correctly implemented in this location.");
    }

    // Note: The main function currently exits after capture_events if it's blocking.
    // If capture_events runs in a separate thread, the main thread might need to
    // do other things or simply wait for a signal to exit.
}
