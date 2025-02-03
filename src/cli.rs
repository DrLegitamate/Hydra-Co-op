use clap::{Arg, Command};
use std::env;
use std::path::PathBuf;
use log::info;

pub fn build_cli() -> Command {
    Command::new("Hydra Co-op")
        .version("1.0")
        .author("Your Name <your.email@example.com>")
        .about("A command-line interface for the application")
        .arg(
            Arg::new("game_executable")
                .short('g')
                .long("game-executable")
                .value_name("PATH")
                .about("Specifies the game executable path")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("instances")
                .short('i')
                .long("instances")
                .value_name("NUM")
                .about("Defines the number of instances (players) to launch")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("input_devices")
                .short('d')
                .long("input-devices")
                .value_name("DEVICES")
                .about("Assigns input devices to each instance (e.g., by providing device file paths or indices)")
                .takes_value(true)
                .required(true)
                .multiple(true),
        )
        .arg(
            Arg::new("layout")
                .short('l')
                .long("layout")
                .value_name("LAYOUT")
                .about("Chooses the desired split-screen layout (horizontal, vertical, custom)")
                .takes_value(true)
                .possible_values(&["horizontal", "vertical", "custom"])
                .required(true),
        )
        .arg(
            Arg::new("debug")
                .short('D')
                .long("debug")
                .about("Enables debug mode for verbose logging")
                .takes_value(false),
        )
}

pub fn parse_args() -> clap::ArgMatches {
    build_cli().get_matches()
}

fn main() {
    let matches = parse_args();

    let game_executable = matches.value_of("game_executable").unwrap();
    let instances = matches.value_of("instances").unwrap();
    let input_devices = matches.values_of("input_devices").unwrap().map(|s| PathBuf::from(s)).collect::<Vec<PathBuf>>();
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
}
