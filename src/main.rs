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
use crate::window_manager::WindowManager;
use crate::input_mux::InputMux;
use std::env;
use log::info;

fn main() {
    // Initialize the logging system
    init();

    // Parse command-line arguments
    let matches = parse_args();

    let game_executable = matches.value_of("game_executable").unwrap();
    let instances = matches.value_of("instances").unwrap().parse::<usize>().unwrap();
    let input_devices = matches.values_of("input_devices").unwrap().collect::<Vec<&str>>();
    let layout = matches.value_of("layout").unwrap();
    let debug = matches.is_present("debug");

    if debug {
        env::set_var("RUST_LOG", "debug");
    } else {
        env::set_var("RUST_LOG", "info");
    }

    info!("Game Executable: {}", game_executable);
    info!("Number of Instances: {}", instances);
    info!("Input Devices: {:?}", input_devices);
    info!("Layout: {}", layout);
    info!("Debug Mode: {}", debug);

    // Load user configuration
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let config = Config::load(&config_path).unwrap_or_else(|_| {
        eprintln!("Failed to load configuration from {}", config_path);
        std::process::exit(1);
    });

    // Launch the required number of game instances
    let game_instances = launch_multiple_game_instances(game_executable.to_string(), instances).unwrap_or_else(|e| {
        eprintln!("Failed to launch game instances: {}", e);
        std::process::exit(1);
    });

    // Set up the virtual network emulator to connect these instances
    let mut net_emulator = NetEmulator::new();
    for (i, instance) in game_instances.iter().enumerate() {
        net_emulator.add_instance(i as u8).unwrap_or_else(|e| {
            eprintln!("Failed to add instance to net emulator: {}", e);
            std::process::exit(1);
        });
    }
    net_emulator.start_relay();

    // Adjust the windows using the window management module to arrange them in the selected split-screen layout
    let window_manager = WindowManager::new().unwrap_or_else(|e| {
        eprintln!("Failed to initialize window manager: {}", e);
        std::process::exit(1);
    });
    let windows = game_instances.iter().map(|instance| instance.id()).collect::<Vec<_>>();
    window_manager.set_layout(windows, layout).unwrap_or_else(|e| {
        eprintln!("Failed to set window layout: {}", e);
        std::process::exit(1);
    });

    // Initialize the input multiplexer to route inputs from individual devices to their assigned game instances
    let mut input_mux = InputMux::new();
    input_mux.enumerate_devices().unwrap_or_else(|e| {
        eprintln!("Failed to enumerate input devices: {}", e);
        std::process::exit(1);
    });
    for (i, device) in input_devices.iter().enumerate() {
        input_mux.map_device_to_instance(device, i);
    }
    input_mux.capture_events();

    // If necessary, detect and launch Windows games via Proton using the Proton integration module
    if matches.is_present("proton") {
        for instance in game_instances {
            launch_game(&game_executable).unwrap_or_else(|e| {
                eprintln!("Failed to launch game with Proton: {}", e);
                std::process::exit(1);
            });
        }
    }
}
