//! Hydra Co-op Launcher — binary entry point.
//!
//! Bootstrapping order:
//!  1. Initialize logging (respecting --debug / RUST_LOG).
//!  2. Parse CLI arguments.
//!  3. Load user configuration (config.toml) and adaptive config.
//!  4. Dispatch to GUI (default) or CLI mode.
//!  5. In either mode, run_core_logic() launches instances, starts the
//!     network emulator, arranges windows, and begins input multiplexing.

mod adaptive_config;
mod cli;
mod config;
mod errors;
mod game_detection;
mod gui;
mod input_mux;
mod logging;
mod net_emulator;
mod proton_integration;
mod universal_launcher;
mod window_manager;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use std::{env, io, thread};

use clap::ArgMatches;
use log::{debug, error, info, warn};

use config::Config;
use errors::{HydraError, Result};
use input_mux::{InputAssignment, InputMux};
use logging::init as init_logging;
use net_emulator::NetEmulator;
use universal_launcher::UniversalLauncher;
use window_manager::{Layout, WindowManager};

/// Launches instances, wires up the virtual network, arranges windows, and
/// starts input multiplexing. Callable from both CLI and GUI entry points.
pub(crate) fn run_core_logic(
    game_executable_path: &Path,
    num_instances: usize,
    input_assignments: &[(usize, InputAssignment)],
    layout: Layout,
    use_proton: bool,
    config: &Config,
) -> Result<(NetEmulator, InputMux, UniversalLauncher)> {
    if num_instances == 0 {
        return Err(HydraError::validation(
            "Number of instances must be at least 1",
        ));
    }
    if num_instances > crate::defaults::MAX_INSTANCES {
        return Err(HydraError::validation(format!(
            "Number of instances ({}) exceeds maximum ({})",
            num_instances,
            crate::defaults::MAX_INSTANCES
        )));
    }

    info!(
        "Launching {} instance(s) of {}",
        num_instances,
        game_executable_path.display()
    );
    debug!("layout={:?} use_proton={} assignments={:?}", layout, use_proton, input_assignments);

    // Launch game instances via the universal launcher (handles Proton wineprefixes internally).
    let mut launcher = UniversalLauncher::new();
    let pids = launcher.launch_game_instances(game_executable_path, num_instances, use_proton)?;

    // Initialise the virtual network emulator and register each instance.
    let mut net_emulator = NetEmulator::new();
    let mut emulator_ports: HashMap<u8, u16> = HashMap::new();
    for (i, pid) in pids.iter().enumerate() {
        let id = i as u8;
        match net_emulator.add_instance(id) {
            Ok(port) => {
                emulator_ports.insert(id, port);
                debug!("Instance {} (pid {}) bound to emulator port {}", id, pid, port);
            }
            Err(e) => error!("Failed to register instance {} in net emulator: {}", id, e),
        }
    }

    // Route traffic destined for each instance's configured game port to that
    // instance's emulator socket on localhost.
    for j in 0..num_instances {
        if let (Some(&emulator_port), Some(&game_port)) =
            (emulator_ports.get(&(j as u8)), config.network_ports.get(j))
        {
            let from: SocketAddr = format!("127.0.0.1:{}", game_port)
                .parse()
                .expect("invalid game address");
            let to: SocketAddr = format!("127.0.0.1:{}", emulator_port)
                .parse()
                .expect("invalid emulator address");
            debug!("Mapping {} -> {}", from, to);
            net_emulator.add_mapping(from, to);
        }
    }
    net_emulator.start_relay()?;

    // Arrange game windows according to the selected layout.
    let window_manager = WindowManager::new()?;
    window_manager.set_layout(&pids, layout)?;

    // Initialise the input multiplexer and begin routing events.
    let mut input_mux = InputMux::new();
    input_mux.enumerate_devices()?;
    input_mux.create_virtual_devices(num_instances)?;
    input_mux.capture_events(input_assignments)?;

    info!("Core logic initialised; background services running.");
    Ok((net_emulator, input_mux, launcher))
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        error!("Application panicked: {info}");
        if let Some(location) = info.location() {
            error!("  at {}:{}", location.file(), location.line());
        }
    }));

    if let Err(e) = run_application() {
        error!("Application failed: {e}");
        std::process::exit(1);
    }
}

fn run_application() -> Result<()> {
    // Seed RUST_LOG before the logger is installed so --debug works immediately.
    let debug_flag = *parse_args_for_logging().get_one("debug").unwrap_or(&false);
    if debug_flag {
        env::set_var("RUST_LOG", "debug");
    } else if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    init_logging().map_err(HydraError::Logging)?;
    info!("Starting {} v{}", crate::APP_NAME, crate::APP_VERSION);

    let matches: ArgMatches = cli::build_cli().get_matches();
    let use_gui_flag = matches.get_flag("gui");
    let cli_args_provided = matches.contains_id("game_executable");

    if use_gui_flag || !cli_args_provided {
        run_gui_mode()
    } else {
        run_cli_mode(&matches)
    }
}

fn run_gui_mode() -> Result<()> {
    info!("Starting GUI mode.");

    let available_devices = enumerate_input_devices();
    info!("Found {} usable input device(s).", available_devices.len());

    let config = load_configuration();

    gui::run_gui(available_devices, config)
        .map_err(|e| HydraError::application(format!("GUI failed: {e}")))
}

fn run_cli_mode(matches: &ArgMatches) -> Result<()> {
    info!("Starting CLI mode.");

    let game_executable_path = Path::new(
        matches
            .get_one::<String>("game_executable")
            .expect("game_executable is required in CLI mode"),
    );
    let num_instances = *matches
        .get_one::<u32>("instances")
        .expect("instances is required in CLI mode") as usize;
    let device_names: Vec<&str> = matches
        .get_many::<String>("input_devices")
        .map(|v| v.map(String::as_str).collect())
        .unwrap_or_default();
    let layout_str = matches
        .get_one::<String>("layout")
        .map(String::as_str)
        .unwrap_or("horizontal");
    let layout = Layout::from(layout_str);

    let mut config = load_configuration();

    // Make the config consistent with the CLI inputs before validating. Without
    // this, first-time CLI runs would fail validation because the default
    // config has no game_paths, input_mappings for this player count, or ports.
    config.game_paths = vec![game_executable_path.to_path_buf()];
    if config.input_mappings.len() < num_instances {
        config
            .input_mappings
            .resize(num_instances, "Auto-detect".to_string());
    }
    if config.network_ports.len() < num_instances {
        let start = config.network_ports.last().copied().unwrap_or(7776) + 1;
        for i in config.network_ports.len()..num_instances {
            config.network_ports.push(start + (i - config.network_ports.len()) as u16);
        }
    }

    config.validate()?;
    let use_proton = matches.get_flag("proton") || config.use_proton;

    // Resolve device names to identifiers.
    let available_devices = enumerate_input_devices();
    let mut assignments: Vec<(usize, InputAssignment)> = Vec::new();
    for i in 0..num_instances {
        let assignment = match device_names.get(i) {
            Some(&"Auto-detect") | Some(&"auto") | Some(&"auto-detect") => {
                InputAssignment::AutoDetect
            }
            Some(name) => available_devices
                .iter()
                .find(|d| d.name == *name)
                .cloned()
                .map(InputAssignment::Device)
                .unwrap_or_else(|| {
                    warn!("Device '{}' not found; player {} will have no input", name, i + 1);
                    InputAssignment::None
                }),
            None => InputAssignment::AutoDetect,
        };
        assignments.push((i, assignment));
    }

    let (mut net_emulator, mut input_mux, mut launcher) = run_core_logic(
        game_executable_path,
        num_instances,
        &assignments,
        layout,
        use_proton,
        &config,
    )?;

    info!("Running. Press Ctrl+C to shut down.");
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || {
            info!("Ctrl+C received; initiating shutdown.");
            running.store(false, Ordering::SeqCst);
        })
        .expect("failed to install Ctrl-C handler");
    }

    while running.load(Ordering::SeqCst) {
        if !launcher.any_running() {
            info!("All game instances exited; shutting down.");
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }

    if let Err(e) = net_emulator.stop_relay() {
        error!("Error stopping network relay: {e}");
    }
    if let Err(e) = input_mux.stop_capture() {
        error!("Error stopping input capture: {e}");
    }
    launcher.shutdown_instances();
    Ok(())
}

/// Load the main configuration from disk, falling back to defaults on any
/// non-fatal error.
fn load_configuration() -> Config {
    let config_path = match get_config_path() {
        Ok(p) => p,
        Err(e) => {
            warn!("Could not determine config path: {}. Using defaults.", e);
            return Config::default_config();
        }
    };

    match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(config::ConfigError::IoError(io_err)) if io_err.kind() == io::ErrorKind::NotFound => {
            warn!("Config not found at {}; using defaults.", config_path.display());
            Config::default_config()
        }
        Err(e) => {
            error!("Failed to load config from {}: {}", config_path.display(), e);
            Config::default_config()
        }
    }
}

fn enumerate_input_devices() -> Vec<input_mux::DeviceIdentifier> {
    let mut mux = InputMux::new();
    match mux.enumerate_devices() {
        Ok(()) => mux.get_available_devices(),
        Err(e) => {
            error!("Failed to enumerate input devices: {}", e);
            Vec::new()
        }
    }
}

pub(crate) fn get_config_path() -> Result<PathBuf> {
    if let Ok(path) = env::var("CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }
    let dir = crate::utils::get_config_dir()?;
    crate::utils::ensure_dir_exists(&dir)?;
    Ok(dir.join("config.toml"))
}

// Early pass to pick up --debug before the full parser runs (which would
// otherwise error on missing args).
fn parse_args_for_logging() -> ArgMatches {
    use clap::{Arg, Command};
    Command::new("hydra-coop-launcher")
        .arg(Arg::new("debug").long("debug").action(clap::ArgAction::SetTrue))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .ignore_errors(true)
        .get_matches()
}

// ---------------------------------------------------------------------------
// The items below are duplicated from lib.rs so that the binary can compile as
// a standalone target (the binary cannot reference items from the library
// crate directly without going through `hydra_coop_launcher::…`). Keep these
// in sync with lib.rs.
// ---------------------------------------------------------------------------

pub(crate) const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const APP_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub(crate) const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

pub(crate) mod defaults {
    pub const MAX_INSTANCES: usize = 8;
}

pub(crate) mod utils {
    use crate::errors::{HydraError, Result};
    use std::path::{Path, PathBuf};

    pub fn get_config_dir() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join("hydra-coop"))
            .ok_or_else(|| HydraError::application("Could not determine config directory"))
    }

    pub fn ensure_dir_exists(path: &Path) -> Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}
