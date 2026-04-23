use clap::{Arg, Command};

/// Builds the Clap Command structure for the application.
pub fn build_cli() -> Command {
    Command::new(crate::APP_NAME)
        .version(crate::APP_VERSION)
        .author(crate::APP_AUTHORS)
        .about(crate::APP_DESCRIPTION)
        .long_about("A comprehensive tool for setting up local split-screen co-operative gameplay on Linux. \
                    Manages multiple game instances, routes input from physical devices to virtual devices, \
                    emulates UDP network traffic between instances, and arranges game windows automatically.")
        .arg(
            Arg::new("game_executable")
                .short('g')
                .long("game-executable")
                .value_name("PATH")
                .help("Specifies the path to the game executable") // Use .help() instead of .about() for arguments
                .required(false), // Made optional since GUI mode doesn't require it
        )
        .arg(
            Arg::new("instances")
                .short('i')
                .long("instances")
                .value_name("NUM")
                .help("Defines the number of game instances (players) to launch")
                .required(false) // Made optional since GUI mode doesn't require it
                // Add validation to ensure the value is a positive integer
                .value_parser(clap::value_parser!(u32).range(1..=(crate::defaults::MAX_INSTANCES as i64))),
        )
        .arg(
            Arg::new("input_devices")
                .short('d')
                .long("input-devices")
                .value_name("DEVICES")
                .help("Assigns input devices to each instance (e.g., by providing device names or identifiers). Provide multiple times for multiple devices.") // Clarify how to provide multiple values
                .required(false) // Made optional since GUI mode doesn't require it
                .action(clap::ArgAction::Append), // Use Append to collect multiple values into a Vec
        )
        .arg(
            Arg::new("layout")
                .short('l')
                .long("layout")
                .value_name("LAYOUT")
                .help("Chooses the desired split-screen layout")
                .required(false) // Made optional since GUI mode doesn't require it
                .value_parser(["horizontal", "vertical", "custom"]), // Simpler way to define possible values
        )
        .arg(
            Arg::new("proton")
                .short('p')
                .long("proton")
                .help("Use Proton to launch Windows games")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("gui")
                .long("gui")
                .help("Launch the graphical user interface")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("PATH")
                .help("Path to configuration file")
                .env("CONFIG_PATH"),
        )
        .arg(
            Arg::new("debug")
                .short('D')
                .long("debug")
                .help("Enables debug mode for verbose logging")
                .action(clap::ArgAction::SetTrue), // Use SetTrue for boolean flags
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::Count),
        )
}

// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_no_arguments_is_ok() {
        // All CLI args are optional (GUI mode launches with none), so parsing with
        // no args must succeed.
        let cmd = build_cli();
        let result = cmd.try_get_matches_from(vec![command_name()]);
        assert!(result.is_ok(), "Parsing with no arguments should succeed");
    }

     #[test]
    fn test_valid_arguments() {
        let cmd = build_cli();
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
         let cmd = build_cli();
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
         let cmd = build_cli();
          let result = cmd.try_get_matches_from(vec![
             command_name(),
             "-g", "/path/to/game",
             "-i", "2",
             "-d", "device",
             "-l", "diagonal", // Invalid layout
         ]);
         assert!(result.is_err());
         let err = result.unwrap_err();
         assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
     }


    // Add more tests for various argument combinations and edge cases
}
