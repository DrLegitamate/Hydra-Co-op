use std::path::Path;
use std::process::{Command, Child};
use std::io;
use log::{error, info};
use std::env;
use std::fs;
use std::io::Write; // Corrected import

/// Launches a single game instance.
///
/// # Arguments
///
/// * `executable_path` - A path to the game executable.
/// * `working_directory` - A Path that holds the working directory for the game process.
///
/// # Returns
///
/// * `Result<Child, io::Error>` - A Result containing the handle to the child process or an error if the process fails to start.
pub fn launch_game_instance(executable_path: &Path, working_directory: &Path) -> Result<Child, io::Error> {
    // Log the start of the game instance launch
    info!("Launching game instance with executable: {}", executable_path.display());
    info!("Setting working directory to: {}", working_directory.display());

    // Ensure the working directory exists
    if let Err(e) = fs::create_dir_all(working_directory) {
        error!("Failed to create working directory {}: {}", working_directory.display(), e);
        return Err(e); // Propagate the error
    }


    // Create a new command to spawn the game process
    let mut command = Command::new(executable_path);

    // Set the working directory for the command
    command.current_dir(working_directory);

    // Spawn the game process
    // Note: By default, stdout and stderr are inherited from the parent process.
    // If you need to capture them, use .stdout(Stdio::piped()) and .stderr(Stdio::piped()).
    let child = command.spawn()?;

    // Log successful process start
    info!("Game instance launched successfully with PID: {}", child.id());

    // Return the handle to the child process
    Ok(child)
}

pub fn launch_multiple_game_instances(executable_path: &Path, num_instances: usize) -> Result<Vec<Child>, io::Error> {
    let mut children = Vec::new();

    for i in 0..num_instances {
        // Create a unique working directory for each instance
        let working_directory_name = format!("instance_{}", i);
        let working_directory = Path::new(&working_directory_name);

        // Ensure the working directory exists
        if let Err(e) = fs::create_dir_all(&working_directory) {
            error!("Failed to create working directory {}: {}", working_directory.display(), e);
            // Depending on requirements, you might continue or return an error here.
            // Returning the error for a single instance failure seems reasonable for a launcher.
            return Err(e);
        }

        // Set unique environment variables for each instance
        // Example: Assigning a potentially unique port number
        let instance_port = format!("808{}", i); // Simple example, may need more robust port allocation

        // Log the start of the game instance launch
        info!("Launching game instance {} with executable: {}", i, executable_path.display());
        info!("Setting working directory to: {}", working_directory.display());
        info!("Setting environment variable INSTANCE_PORT={}", instance_port);


        // Create a new command to spawn the game process
        let mut command = Command::new(executable_path);

        // Set the working directory for the command
        command.current_dir(&working_directory);

        // Set environment variables for the command
        command.env("INSTANCE_PORT", &instance_port);

        // Spawn the game process
        // Note: By default, stdout and stderr are inherited from the parent process.
        let child = command.spawn()?;

        // Log successful process start
        info!("Game instance {} launched successfully with PID: {}", i, child.id());

        // Return the handle to the child process
        children.push(child);
    }

    Ok(children)
}

// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::fs;
    use tempfile::tempdir; // Using tempfile crate for temporary directories
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt; // For setting execute permissions on Unix

    // Note: Writing robust cross-platform tests for process execution can be complex.
    // This is a basic example for Unix-like systems.

    #[test]
    #[cfg(unix)] // Conditionally compile test for Unix-like systems
    fn test_launch_game_instance_success() {
        // Create a temporary directory for test files
        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let dummy_executable_path = temp_test_dir.path().join("dummy_game.sh");
        let working_dir_path = temp_test_dir.path().join("working_dir");

        // Create a simple executable shell script
        let script_content = b"#!/bin/sh\nexit 0\n";
        let mut file = fs::File::create(&dummy_executable_path).expect("Failed to create dummy executable file");
        file.write_all(script_content).expect("Failed to write to dummy executable file");

        // Make the script executable
        let mut perms = fs::metadata(&dummy_executable_path).expect("Failed to get dummy executable permissions").permissions();
        perms.set_mode(0o755); // Read, write, execute for owner, read and execute for group and others
        fs::set_permissions(&dummy_executable_path, perms).expect("Failed to set dummy executable permissions");

        // Ensure working directory exists
        fs::create_dir_all(&working_dir_path).expect("Failed to create test working directory");

        // Launch the dummy game instance
        let child_result = launch_game_instance(&dummy_executable_path, &working_dir_path);

        // Assert that the launch was successful
        assert!(child_result.is_ok(), "Game instance launch failed: {:?}", child_result.err());

        // Wait for the child process to exit and check its status
        let mut child = child_result.unwrap();
        let exit_status = child.wait().expect("Failed to wait on child process");

        // Assert that the process exited successfully (exit code 0)
        assert!(exit_status.success(), "Dummy game process failed to exit successfully: {:?}", exit_status);

        // temp_test_dir is automatically cleaned up when it goes out of scope
    }

     #[test]
     #[cfg(unix)] // Conditionally compile test for Unix-like systems
     fn test_launch_game_instance_no_executable() {
         let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
         let non_existent_executable = temp_test_dir.path().join("non_existent_game.sh");
         let working_dir_path = temp_test_dir.path().join("working_dir");

         fs::create_dir_all(&working_dir_path).expect("Failed to create test working directory");

         // Attempt to launch a non-existent executable
         let child_result = launch_game_instance(&non_existent_executable, &working_dir_path);

         // Assert that the launch failed with an appropriate error
         assert!(child_result.is_err(), "Game instance launch unexpectedly succeeded");
         let err = child_result.err().unwrap();
         // Check for a specific OS error if possible, or a general I/O error
         // The exact error kind might vary between OS versions/configurations
         // assert_eq!(err.kind(), io::ErrorKind::NotFound); // Example for checking a specific kind
     }


    #[test]
    #[cfg(unix)] // Conditionally compile test for Unix-like systems
    fn test_launch_multiple_game_instances_success() {
        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let dummy_executable_path = temp_test_dir.path().join("dummy_game.sh");

        // Create a simple executable shell script
        let script_content = b"#!/bin/sh\nexit 0\n";
        let mut file = fs::File::create(&dummy_executable_path).expect("Failed to create dummy executable file");
        file.write_all(script_content).expect("Failed to write to dummy executable file");

        // Make the script executable
        let mut perms = fs::metadata(&dummy_executable_path).expect("Failed to get dummy executable permissions").permissions();
        perms.set_mode(0o755); // Read, write, execute for owner, read and execute for group and others
        fs::set_permissions(&dummy_executable_path, perms).expect("Failed to set dummy executable permissions");


        let num_instances = 3;
        let children_result = launch_multiple_game_instances(&dummy_executable_path, num_instances);

        // Assert that the launch was successful
        assert!(children_result.is_ok(), "Launching multiple instances failed: {:?}", children_result.err());

        let mut children = children_result.unwrap();
        assert_eq!(children.len(), num_instances);

        // Wait for each child process to exit
        for mut child in children {
            let exit_status = child.wait().expect("Failed to wait on child process");
            assert!(exit_status.success(), "One of the dummy game processes failed");
        }

         // Verify working directories were created
         for i in 0..num_instances {
             let expected_dir = Path::new(&format!("instance_{}", i));
             assert!(expected_dir.exists(), "Working directory {} was not created", expected_dir.display());
              // Clean up created instance directories
             fs::remove_dir_all(expected_dir).expect("Failed to clean up instance directory");
         }

        // temp_test_dir is automatically cleaned up
    }

    // Add more tests for edge cases like directory creation failures, invalid arguments, etc.
}
