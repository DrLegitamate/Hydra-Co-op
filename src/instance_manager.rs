use std::path::{Path, PathBuf}; // Import PathBuf
use std::process::{Command, Child};
use std::io;
use log::{error, info, warn, debug}; // Import debug and warn
use std::env;
use std::fs;
use std::io::Write;

// Import necessary items from proton_integration
use crate::proton_integration::{ProtonError, find_proton_path, prepare_command_with_proton, is_windows_binary};
use std::error::Error; // Import Error trait

// Custom error type for Instance Manager operations
#[derive(Debug)]
pub enum InstanceManagerError {
    IoError(io::Error),
    ProtonError(ProtonError), // Include Proton-specific errors
    GenericError(String),
    ProtonPathNotFound, // Specific error for when Proton is requested but not found
    WindowsBinaryCheckError(ProtonError), // Error during Windows binary check
}

impl std::fmt::Display for InstanceManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InstanceManagerError::IoError(e) => write!(f, "Instance manager I/O error: {}", e),
            InstanceManagerError::ProtonError(e) => write!(f, "Proton integration error: {}", e),
            InstanceManagerError::GenericError(msg) => write!(f, "Instance manager error: {}", msg),
            InstanceManagerError::ProtonPathNotFound => write!(f, "Proton executable not found"),
            InstanceManagerError::WindowsBinaryCheckError(e) => write!(f, "Windows binary check error: {}", e),
        }
    }
}

impl Error for InstanceManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InstanceManagerError::IoError(e) => Some(e),
            InstanceManagerError::ProtonError(e) => Some(e),
             InstanceManagerError::WindowsBinaryCheckError(e) => Some(e),
            _ => None,
        }
    }
}

// Implement From conversions for easier error propagation
impl From<io::Error> for InstanceManagerError {
    fn from(err: io::Error) -> Self {
        InstanceManagerError::IoError(err)
    }
}

impl From<ProtonError> for InstanceManagerError {
    fn from(err: ProtonError) -> Self {
        InstanceManagerError::ProtonError(err)
    }
}


/// Launches a single game instance.
/// This function is now less likely to be used directly for multi-instance
/// scenarios handled by `launch_multiple_game_instances`.
///
/// # Arguments
///
/// * `executable_path` - A path to the game executable.
/// * `working_directory` - A Path that holds the working directory for the game process.
///
/// # Returns
///
/// * `Result<Child, InstanceManagerError>` - A Result containing the handle to the child process or an error if the process fails to start.
pub fn launch_game_instance(executable_path: &Path, working_directory: &Path) -> Result<Child, InstanceManagerError> {
    // Log the start of the game instance launch
    info!("Launching single game instance with executable: {}", executable_path.display());
    info!("Setting working directory to: {}", working_directory.display());

    // Ensure the working directory exists
    if let Err(e) = fs::create_dir_all(working_directory) {
        error!("Failed to create working directory {}: {}", working_directory.display(), e);
        return Err(InstanceManagerError::IoError(e)); // Map to custom error
    }

    // Create a new command to spawn the game process
    let mut command = Command::new(executable_path);

    // Set the working directory for the command
    command.current_dir(working_directory);

    // Spawn the game process
    // Note: By default, stdout and stderr are inherited from the parent process.
    // If you need to capture them, use .stdout(Stdio::piped()) and .stderr(Stdio::piped()).
    let child = command.spawn().map_err(InstanceManagerError::IoError)?; // Map to custom error

    // Log successful process start
    info!("Single game instance launched successfully with PID: {}", child.id());

    // Return the handle to the child process
    Ok(child)
}

/// Launches multiple game instances, optionally using Proton.
///
/// # Arguments
///
/// * `executable_path` - A path to the game executable (Windows binary if using Proton).
/// * `num_instances` - The number of instances to launch.
/// * `use_proton` - Whether to launch the game using Proton.
/// * `base_wineprefix_dir` - The base directory for creating unique WINEPREFIXes for each instance if using Proton.
///
/// # Returns
///
/// * `Result<Vec<Child>, InstanceManagerError>` - A vector of Child process handles or an error.
pub fn launch_multiple_game_instances(
    executable_path: &Path,
    num_instances: usize,
    use_proton: bool,
    base_wineprefix_dir: &Path,
) -> Result<Vec<Child>, InstanceManagerError> {
    info!("Attempting to launch {} game instances.", num_instances);
    debug!("Executable path: {}", executable_path.display());
    debug!("Use Proton: {}", use_proton);
    debug!("Base WINEPREFIX directory: {}", base_wineprefix_dir.display());


    let proton_path_option = if use_proton {
        // Find Proton once before the launch loop
        info!("Proton launch requested. Finding Proton executable...");
        match find_proton_path() {
            Ok(path) => {
                info!("Proton executable found at: {}", path.display());
                Some(path)
            }
            Err(e @ ProtonError::ProtonNotFound(_)) => {
                error!("Failed to find Proton path: {}", e);
                // Return a specific error indicating Proton was not found when requested
                return Err(InstanceManagerError::ProtonPathNotFound);
            }
            Err(e) => {
                 error!("Error while trying to find Proton path: {}", e);
                 // Return other Proton errors encountered during path finding
                 return Err(InstanceManagerError::ProtonError(e));
            }
        }
    } else {
        None
    };

    // Optional: Check if the game executable is a Windows binary if use_proton is true
    if use_proton {
        debug!("Checking if game executable is a Windows binary...");
        match is_windows_binary(executable_path) {
            Ok(true) => info!("Game executable appears to be a Windows binary."),
            Ok(false) => {
                warn!("Game executable '{}' does not appear to be a Windows binary based on MZ header check. Launching with Proton might fail.", executable_path.display());
                // Decide if this warning is sufficient or if it should be a fatal error.
                // For now, log a warning and proceed.
            }
            Err(e) => {
                 error!("Error checking if game executable is Windows binary: {}", e);
                 // Decide if an error during the check should prevent launch.
                 // For now, log the error and proceed.
                 // return Err(InstanceManagerError::WindowsBinaryCheckError(e));
            }
        }
    }


    let mut children = Vec::new();

    for i in 0..num_instances {
        // Create a unique working directory for each instance
        let working_directory_name = format!("instance_{}", i);
        let working_directory = Path::new(&working_directory_name);

        // Ensure the working directory exists
        if let Err(e) = fs::create_dir_all(&working_directory) {
            error!("Failed to create working directory {}: {}", working_directory.display(), e);
            // Depending on requirements, you might continue or return an error here.
            return Err(InstanceManagerError::IoError(e)); // Map to custom error and return
        }

        let mut command_to_spawn: Command;

        if let Some(proton_path) = &proton_path_option {
            // Launch with Proton for this instance
            info!("Preparing to launch instance {} with Proton.", i);
            match prepare_command_with_proton(executable_path, proton_path, i, base_wineprefix_dir) {
                Ok(command) => {
                    command_to_spawn = command;
                }
                Err(e) => {
                    error!("Failed to prepare Proton command for instance {}: {}", i, e);
                    // Decide how to handle this failure: skip instance, return error, etc.
                    // Returning the error for a single instance preparation failure seems reasonable.
                     return Err(InstanceManagerError::ProtonError(e)); // Map and return Proton error
                }
            }
        } else {
            // Launch natively for this instance
            info!("Preparing to launch instance {} natively.", i);
            command_to_spawn = Command::new(executable_path);

            // Set environment variables for native launch (if any specific ones are needed)
            // Example: Assigning a potentially unique port number as an environment variable
            let instance_port = format!("808{}", i); // Simple example
            command_to_spawn.env("HYDRA_INSTANCE_PORT", &instance_port); // Use a more specific env var name
            debug!("Setting environment variable HYDRA_INSTANCE_PORT={} for instance {}.", instance_port, i);

            // Set other environment variables that apply to native launch
        }

        // Set working directory and environment variables that apply to both native and Proton launches
        // Note: WINEPREFIX is handled by prepare_command_with_proton if using Proton.
        command_to_spawn.current_dir(&working_directory);
        // Example of an environment variable that might be useful for both native and Proton instances
        command_to_spawn.env("HYDRA_INSTANCE_INDEX", i.to_string());
         debug!("Setting environment variable HYDRA_INSTANCE_INDEX={} for instance {}.", i, i);


        // Spawn the process
        debug!("Spawning command: {:?}", command_to_spawn);
        let child = command_to_spawn.spawn().map_err(InstanceManagerError::IoError)?; // Map spawn error


        // Log successful process start
        info!("Game instance {} launched successfully with PID: {}", i, child.id());

        // Add the handle to the child process vector
        children.push(child);
    }

    info!("Finished attempting to launch {} instances.", num_instances);
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

    // Note: Testing Proton integration here is complex as it requires a dummy
    // 'proton' executable and a controlled environment for WINEPREFIX.
    // These tests primarily focus on the native launch logic and error handling.
    // Separate integration tests would be needed for Proton scenarios.

    // Helper function to create a dummy executable
    #[cfg(unix)]
    fn create_dummy_executable(dir: &Path, name: &str) -> PathBuf {
        let exec_path = dir.join(name);
        let script_content = b"#!/bin/sh\nexit 0\n"; // Simple script that exits successfully
        let mut file = fs::File::create(&exec_path).expect("Failed to create dummy executable file");
        file.write_all(script_content).expect("Failed to write to dummy executable file");

        // Make the script executable
        let mut perms = fs::metadata(&exec_path).expect("Failed to get dummy executable permissions").permissions();
        perms.set_mode(0o755); // Read, write, execute for owner, read and execute for group and others
        fs::set_permissions(&exec_path, perms).expect("Failed to set dummy executable permissions");

        exec_path
    }


    #[test]
    #[cfg(unix)] // Conditionally compile test for Unix-like systems
    fn test_launch_single_game_instance_success() {
        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let dummy_executable_path = create_dummy_executable(temp_test_dir.path(), "dummy_game.sh");
        let working_dir_path = temp_test_dir.path().join("working_dir");

        fs::create_dir_all(&working_dir_path).expect("Failed to create test working directory");

        let child_result = launch_game_instance(&dummy_executable_path, &working_dir_path);

        assert!(child_result.is_ok(), "Single game instance launch failed: {:?}", child_result.err());

        let mut child = child_result.unwrap();
        let exit_status = child.wait().expect("Failed to wait on child process");
        assert!(exit_status.success(), "Dummy game process failed to exit successfully: {:?}", exit_status);
    }

     #[test]
     #[cfg(unix)] // Conditionally compile test for Unix-like systems
     fn test_launch_single_game_instance_no_executable() {
         let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
         let non_existent_executable = temp_test_dir.path().join("non_existent_game.sh");
         let working_dir_path = temp_test_dir.path().join("working_dir");

         fs::create_dir_all(&working_dir_path).expect("Failed to create test working directory");

         let child_result = launch_game_instance(&non_existent_executable, &working_dir_path);

         assert!(child_result.is_err(), "Single game instance launch unexpectedly succeeded");
         let err = child_result.err().unwrap();
         match err {
              InstanceManagerError::IoError(io_err) => {
                   assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
              }
              _ => panic!("Expected IoError but got: {:?}", err),
         }
     }


    #[test]
    #[cfg(unix)] // Conditionally compile test for Unix-like systems
    fn test_launch_multiple_game_instances_native_success() {
        let temp_test_dir = tempdir().expect("Failed to create temporary test directory");
        let dummy_executable_path = create_dummy_executable(temp_test_dir.path(), "dummy_game.sh");
        let base_wineprefix_dir = temp_test_dir.path().join("wineprefixes"); // Dummy path, not used in native launch

        let num_instances = 3;
        let children_result = launch_multiple_game_instances(
            &dummy_executable_path,
            num_instances,
            false, // Not using Proton
            &base_wineprefix_dir,
        );

        assert!(children_result.is_ok(), "Launching multiple native instances failed: {:?}", children_result.err());

        let mut children = children_result.unwrap();
        assert_eq!(children.len(), num_instances);

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
    }

    // Add tests for:
    // - launch_multiple_game_instances with use_proton = true (requires dummy proton and mocking/careful environment)
    // - Failure to create working directory for multiple instances
    // - Error finding Proton when use_proton is true
    // - Error preparing Proton command
}
