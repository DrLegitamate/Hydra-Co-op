use std::path::Path;
use std::process::{Command, Child};
use std::io;
use log::{error, info};
use std::env;
use std::fs;
use std::io::{self, Write};

mod net_emulator;

/// Launches a single game instance.
///
/// # Arguments
///
/// * `executable_path` - A string slice that holds the path to the game executable.
/// * `working_directory` - A Path that holds the working directory for the game process.
///
/// # Returns
///
/// * `Result<Child, io::Error>` - A Result containing the handle to the child process or an error if the process fails to start.
pub fn launch_game_instance(executable_path: String, working_directory: &Path) -> Result<Child, io::Error> {
    // Log the start of the game instance launch
    info!("Launching game instance with executable: {}", executable_path);

    // Create a new command to spawn the game process
    let mut command = Command::new(executable_path);

    // Set the working directory for the command
    command.current_dir(working_directory);

    // Spawn the game process, capturing stdout and stderr
    let child = command.spawn()?;

    // Log successful process start
    info!("Game instance launched successfully with PID: {}", child.id());

    // Return the handle to the child process
    Ok(child)
}

pub fn launch_multiple_game_instances(executable_path: String, num_instances: usize) -> Result<Vec<Child>, io::Error> {
    let mut children = Vec::new();

    for i in 0..num_instances {
        // Create a unique working directory for each instance
        let working_directory = format!("instance_{}", i);
        fs::create_dir_all(&working_directory)?;

        // Set unique environment variables for each instance
        let instance_port = format!("808{}", i);

        // Log the start of the game instance launch
        info!("Launching game instance {} with executable: {}", i, executable_path);

        // Create a new command to spawn the game process
        let mut command = Command::new(&executable_path);

        // Set the working directory for the command
        command.current_dir(&working_directory);

        // Set environment variables for the command
        command.env("INSTANCE_PORT", &instance_port);

        // Spawn the game process, capturing stdout and stderr
        let child = command.spawn()?;

        // Log successful process start
        info!("Game instance {} launched successfully with PID: {}", i, child.id());

        // Return the handle to the child process
        children.push(child);
    }

    Ok(children)
}

fn main() {
    // Initialize the logger
    env_logger::init();

    // Example usage of the net_emulator module
    let emulator = net_emulator::NetEmulator::new();

    // Add instances
    emulator.add_instance(1).unwrap();
    emulator.add_instance(2).unwrap();

    // Add mappings
    use std::net::SocketAddr;
    let src_addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
    let dst_addr: SocketAddr = "127.0.0.1:8082".parse().unwrap();
    emulator.add_mapping(src_addr, dst_addr);

    // Start relay
    emulator.start_relay();

    // Launch game instances
    let executable_path = "/usr/bin/game_executable".to_string();
    let working_directory = Path::new("/tmp/game_working_directory");
    launch_game_instance(executable_path.clone(), working_directory).unwrap();

    let num_instances = 3;
    launch_multiple_game_instances(executable_path, num_instances).unwrap();
}
