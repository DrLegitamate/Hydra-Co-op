use clap::{Arg, Command, ArgMatches};
use std::path::PathBuf; // Keep if you need PathBuf in this module for some reason, but not needed for parsing Vec<&str>
use log::debug; // Use debug for cli parsing details

/// Builds the Clap Command structure for the application.
pub fn build_cli() -> Command {
    Command::new("Hydra Co-op")
        .version("1.0") // Consider getting the version from Cargo.toml using env!("CARGO_PKG_VERSION")
        .author(env!("CARGO_PKG_AUTHORS")) // Get authors from Cargo.toml
        .about(env!("CARGO_PKG_DESCRIPTION")) // Get description from Cargo.toml
        .arg(
            Arg::new("game_executable")
                .short('g')
                .long("game-executable")
                .value_name("PATH")
                .help("Specifies the path to the game executable") // Use .help() instead of .about() for arguments
                .required(true),
        )
        .arg(
            Arg::new("instances")
                .short('i')
                .long("instances")
                .value_name("NUM")
                .help("Defines the number of game instances (players) to launch")
                .required(true)
                // Add validation to ensure the value is a positive integer
                .value_parser(clap::value_parser!(u32).range(1..)),
        )
        .arg(
            Arg::new("input_devices")
                .short('d')
                .long("input-devices")
                .value_name("DEVICES")
                .help("Assigns input devices to each instance (e.g., by providing device names or identifiers). Provide multiple times for multiple devices.") // Clarify how to provide multiple values
                .required(true) // Requires at least one device
                .action(clap::ArgAction::Append), // Use Append to collect multiple values into a Vec
        )
        .arg(
            Arg::new("layout")
                .short('l')
                .long("layout")
                .value_name("LAYOUT")
                .help("Chooses the desired split-screen layout")
                .required(true)
                .value_parser(["horizontal", "vertical", "custom"]), // Simpler way to define possible values
        )
        .arg(
            Arg::new("debug")
                .short('D')
                .long("debug")
                .help("Enables debug mode for verbose logging")
                .action(clap::ArgAction::SetTrue), // Use SetTrue for boolean flags
        )
}

/// Parses the command-line arguments.
/// Clap's get_matches() will automatically handle help messages and errors
/// for missing or invalid arguments by printing to stderr and exiting.
pub fn parse_args() -> ArgMatches {
    debug!("Parsing command-line arguments...");
    build_cli().get_matches()
}

// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory; // Required for Command::command() in tests

    // Helper function to get the command name for tests
    fn command_name() -> &'static str {
        "hydra-co-op" // Replace with your actual binary name if different
    }


    #[test]
    fn test_cli_build() {
        // Simply checks if the CLI can be built without panicking
        build_cli().debug_assert(); // clap's built-in debug assertion
    }

    #[test]
    fn test_required_arguments() {
        // Test that required arguments are indeed required
        let mut cmd = build_cli();
        // Calling get_matches_from with missing required args should result in an error
        let result = cmd.try_get_matches_from(vec![command_name()]);
        assert!(result.is_err(), "Should fail without required arguments");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelpOnMissingArgOrSubcommand);
    }

     #[test]
    fn test_valid_arguments() {
        let mut cmd = build_cli();
        let matches = cmd.try_get_matches_from(vec![
            command_name(),
            "-g", "/path/to/game",
            "-i", "2",
            "-d", "/dev/input/event0",
            "-d", "/dev/input/event1",
            "-l", "horizontal",
            "-D",
        ]).expect("Valid arguments should be parsed successfully");

        assert_eq!(matches.get_one::<String>("game_executable").map(|s| s.as_str()), Some("/path/to/game"));
        assert_eq!(matches.get_one::<u32>("instances"), Some(&2));
        // clap returns Vec<&String> for multiple values by default if not specified otherwise
        let input_devices: Vec<&String> = matches.get_many("input_devices").expect("input_devices should be present").collect();
        let expected_devices: Vec<String> = vec!["/dev/input/event0".to_string(), "/dev/input/event1".to_string()];
        // Compare collected &Strings with expected Strings
        assert_eq!(input_devices.iter().map(|s| s.as_str()).collect::<Vec<&str>>(), expected_devices.iter().map(|s| s.as_str()).collect::<Vec<&str>>());


        assert_eq!(matches.get_one::<String>("layout").map(|s| s.as_str()), Some("horizontal"));
        assert!(matches.get_flag("debug"));
    }

     #[test]
     fn test_invalid_instances() {
         let mut cmd = build_cli();
          let result = cmd.try_get_matches_from(vec![
             command_name(),
             "-g", "/path/to/game",
             "-i", "abc", // Invalid number
             "-d", "device",
             "-l", "horizontal",
         ]);
         assert!(result.is_err());
         let err = result.unwrap_err();
         assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
     }

      #[test]
     fn test_invalid_layout() {
         let mut cmd = build_cli();
          let result = cmd.try_get_matches_from(vec![
             command_name(),
             "-g", "/path/to/game",
             "-i", "2",
             "-d", "device",
             "-l", "diagonal", // Invalid layout
         ]);
         assert!(result.is_err());
         let err = result.unwrap_err();
         assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
     }


    // Add more tests for various argument combinations and edge cases
}

// The original main function is for testing the module independently.
// The actual application's main function is in src/main.rs.
// #[cfg(not(test))] // Compile this main only when not running tests
// fn main() {
//      // Initialize logger if running this module directly for testing
//      // env_logger::init();
//     let matches = parse_args();

//      // Example of retrieving values with clap 4.0+
//      // Use get_one for single values, get_many for multiple values, get_flag for boolean flags

//     let game_executable: Option<&String> = matches.get_one("game_executable");
//     let instances: Option<&u32> = matches.get_one("instances"); // Assuming value_parser!(u32)
//     let input_devices: Option<clap::parser::Values<'_, String>> = matches.get_many("input_devices"); // Assuming multiple(true) and default String parsing
//     let layout: Option<&String> = matches.get_one("layout");
//     let debug: bool = matches.get_flag("debug");


//      // In your actual main.rs, you would use unwrap() or expect() on required arguments
//      // after calling parse_args(), as clap will exit if they are missing.

//     if debug {
//         // Logging initialization should be in main.rs
//         // env::set_var("RUST_LOG", "debug");
//     } else {
//         // env::set_var("RUST_LOG", "info");
//     }

//     debug!("Parsed Arguments:");
//     debug!("Game Executable: {:?}", game_executable);
//     debug!("Number of Instances: {:?}", instances);
//     debug!("Input Devices: {:?}", input_devices.map(|values| values.collect::<Vec<_>>()));
//     debug!("Layout: {:?}", layout);
//     debug!("Debug Mode: {}", debug);

//      // Note: The main function in cli.rs should ideally just test the parsing logic,
//      // not perform application setup like logging.
// }
