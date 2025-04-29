use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf}; // Import PathBuf
use std::process::{Command, Stdio, Child}; // Import Child
use std::str;
use log::{info, error, warn, debug}; // Import logging macros
use std::error::Error; // Import Error trait

// Custom error type for Proton integration operations
#[derive(Debug)]
pub enum ProtonError {
    IoError(io::Error),
    NotWindowsBinary(PathBuf), // Include the path that wasn't a Windows binary
    ProtonNotFound(String), // Provide context about why Proton wasn't found
    LaunchFailed(String), // Provide context about the launch failure
    GenericError(String),
}

impl std::fmt::Display for ProtonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ProtonError::IoError(e) => write!(f, "Proton integration I/O error: {}", e),
            ProtonError::NotWindowsBinary(path) => write!(f, "File is not a Windows binary: {}", path.display()),
            ProtonError::ProtonNotFound(msg) => write!(f, "Proton not found: {}", msg),
            ProtonError::LaunchFailed(msg) => write!(f, "Proton launch failed: {}", msg),
            ProtonError::GenericError(msg) => write!(f, "Proton integration error: {}", msg),
        }
    }
}

impl Error for ProtonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProtonError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

// Implement From conversions for easier error propagation
impl From<io::Error> for ProtonError {
    fn from(err: io::Error) -> Self {
        ProtonError::IoError(err)
    }
}

/// Checks if the given file is a likely Windows PE (Portable Executable) binary.
/// This is a basic check based on the "MZ" header. It's not foolproof.
fn is_windows_binary(file_path: &Path) -> Result<bool, ProtonError> {
    debug!("Checking if file is a Windows binary: {}", file_path.display());
    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
             debug!("File not found, not a Windows binary check target: {}", file_path.display());
             return Ok(false); // File not found, so not a Windows binary for our purpose
        }
        Err(e) => {
             error!("Failed to open file for Windows binary check: {}: {}", file_path.display(), e);
             return Err(ProtonError::IoError(e)); // Propagate other IO errors
        }
    };

    let mut buffer = [0; 2];
    if file.read_exact(&mut buffer).is_err() {
         debug!("Failed to read enough bytes for MZ check: {}", file_path.display());
        return Ok(false); // Couldn't read, assume not a Windows binary for this check
    }

    let is_pe = buffer == [0x4D, 0x5A]; // "MZ" signature
     debug!("MZ signature check for {}: {}", file_path.display(), is_pe);
    Ok(is_pe)
}

/// Attempts to find the Proton executable path.
/// This is a complex task as Proton installations vary.
/// Strategies:
/// 1. Check PROTON_PATH environment variable.
/// 2. Search common Steam Library folders (requires knowing Steam's structure).
/// 3. Rely on user configuration (e.g., in config.toml).
/// 4. Use tools like `steam-run` or `protondb` if available.
///
/// # Returns
///
/// * `Result<PathBuf, ProtonError>` - The path to the Proton executable if found.
fn find_proton_path() -> Result<PathBuf, ProtonError> {
    info!("Attempting to find Proton executable.");

    // 1. Check PROTON_PATH environment variable
    if let Ok(proton_path_env) = env::var("PROTON_PATH") {
        let path = PathBuf::from(proton_path_env);
        if path.exists() {
             info!("Found Proton using PROTON_PATH environment variable: {}", path.display());
            return Ok(path);
        } else {
             warn!("PROTON_PATH environment variable set, but path does not exist: {}", path.display());
        }
    }

    // 2. Implement searching common Steam Library folders (Requires knowledge of Steam paths and structures)
    // This is highly dependent on the user's system and Steam installation.
    // Example (Illustrative - requires implementing actual search logic):
    /*
    info!("Searching common Steam Library folders for Proton...");
    if let Some(steam_path) = dirs::data_dir().map(|d| d.join("Steam")) { // Example: Using dirs crate for common data dir
         // Implement recursive search within steam_path/steamapps/common/Proton* for proton executable
         // This requires traversing directories and checking for the 'proton' binary.
         warn!("Searching Steam Library folders is not yet implemented.");
    }
    */

     // 3. Rely on user configuration (e.g., from the loaded Config)
     // This would involve passing the Config struct to this function or having
     // a separate function to get the Proton path from config.
     // Example:
     /*
     if let Some(config_proton_path) = config.proton_path { // Assuming a proton_path field in your Config
          let path = PathBuf::from(config_proton_path);
          if path.exists() {
              info!("Found Proton using configuration file: {}", path.display());
              return Ok(path);
          } else {
              warn!("Proton path specified in configuration does not exist: {}", path.display());
          }
     }
     */


    // If no Proton path found by implemented methods
    error!("Proton executable not found through environment variable or default locations.");
    Err(ProtonError::ProtonNotFound(
        "Proton executable not found. Please ensure it's installed and consider setting the PROTON_PATH environment variable or adding it to your configuration.".to_string()
    ))
}

/// Prepares a Command to be run with Proton.
/// This function should be called by the instance manager when launching a game
/// that requires Proton.
///
/// # Arguments
///
/// * `game_path` - The path to the Windows game executable.
/// * `proton_path` - The path to the Proton executable.
/// * `instance_index` - The index of the game instance (0, 1, 2, ...). Used for WINEPREFIX.
/// * `base_wineprefix_dir` - The base directory where WINEPREFIXes will be created for each instance.
///
/// # Returns
///
/// * `Result<Command, ProtonError>` - A configured Command ready to be spawned.
pub fn prepare_command_with_proton(
    game_path: &Path,
    proton_path: &Path,
    instance_index: usize,
    base_wineprefix_dir: &Path,
) -> Result<Command, ProtonError> {
    info!("Preparing command to launch game with Proton: {}", game_path.display());
    debug!("Using Proton executable: {}", proton_path.display());
    debug!("Instance index: {}", instance_index);

    // Construct the WINEPREFIX path for this instance
    // Each instance needs a unique WINEPREFIX to avoid conflicts
    let wineprefix = base_wineprefix_dir.join(format!("instance_{}_wineprefix", instance_index));
    debug!("Using WINEPREFIX: {}", wineprefix.display());

    // Ensure the WINEPREFIX directory exists
    if let Err(e) = std::fs::create_dir_all(&wineprefix) {
         error!("Failed to create WINEPREFIX directory {}: {}", wineprefix.display(), e);
         return Err(ProtonError::IoError(e));
    }


    let mut command = Command::new(proton_path);
    command.arg("run"); // Proton often uses 'run' or the executable name directly

    // Add the game executable as an argument to Proton
    command.arg(game_path);

    // Set essential environment variables for Proton
    command.env("WINEPREFIX", &wineprefix);
    command.env("PROTON_LOG", "1"); // Enable Proton logging (logs will be in WINEPREFIX)

    // You might need to set other environment variables depending on the game and Proton version
    // Examples: WINEDEBUG, WINEESYNC, WINEFSYNC, VKD3D_HUD, etc.

    // Configure standard I/O for the launched process.
    // Inherit is usually fine for games, but piped would be needed to capture output.
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    debug!("Constructed Proton command: {:?}", command);

    Ok(command)
}

// The top-level launch_game function might be removed or refactored.
// The responsibility of launching should be with the instance manager.
/*
/// Main function to handle launching a Windows game via Proton.
/// This function's role might change to be more of a helper or
/// discovery function called by the instance manager.
pub fn launch_game(game_path: &Path) -> Result<Child, ProtonError> { // Changed return type to Child
     info!("Attempting to launch game with Proton: {}", game_path.display());

     // Check if the file is a Windows binary (optional but helpful)
     match is_windows_binary(game_path) {
         Ok(true) => info!("File appears to be a Windows binary."),
         Ok(false) => {
             warn!("File '{}' does not appear to be a Windows binary based on MZ header check.", game_path.display());
             // Decide if this should be a fatal error or a warning allowing launch anyway.
             // For now, let's treat it as a potential issue but allow launch.
             // return Err(ProtonError::NotWindowsBinary(game_path.to_path_buf()));
         }
         Err(e) => {
              error!("Error checking if file is Windows binary: {}", e);
              // Decide if this error should prevent launch
              // return Err(e);
         }
     }


     let proton_path = find_proton_path()?; // Use the improved find_proton_path

     // This top-level function can't easily manage WINEPREFIX for multiple instances.
     // This logic needs to be integrated into the instance_manager's launch loop.
     // The WINEPREFIX base directory needs to be decided (e.g., in config or a temp dir).
     let base_wineprefix_dir = PathBuf::from("/tmp/hydra_wineprefixes"); // Example base directory

     // This part of the logic is more suitable for the instance manager
     // as it ties the launched Child process to a specific instance index and WINEPREFIX.
     error!("The top-level launch_game function in proton_integration is not suitable for multi-instance WINEPREFIX management and should be refactored.");
     return Err(ProtonError::GenericError("Proton launch logic needs to be integrated with instance manager.".to_string()));


     // Example of how prepare_command_with_proton would be used (in instance_manager)
     // let instance_index = 0; // This would come from the loop in instance_manager
     // let mut command = prepare_command_with_proton(game_path, &proton_path, instance_index, &base_wineprefix_dir)?;
     // let child = command.spawn().map_err(|e| ProtonError::LaunchFailed(format!("Failed to spawn Proton command: {}", e)))?;
     // info!("Proton process launched with PID: {}", child.id());
     // Ok(child) // Return the Child process
}
*/

// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir; // Add tempfile = "3.2" to your Cargo.toml
    use std::fs;

    #[test]
    fn test_is_windows_binary_mz_header() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let test_file = temp_dir.path().join("test_mz.bin");
        fs::write(&test_file, b"MZ This is a test").expect("Failed to write test file");
        let is_binary = is_windows_binary(&test_file).expect("Error checking binary type");
        assert!(is_binary);
    }

     #[test]
    fn test_is_windows_binary_other_header() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let test_file = temp_dir.path().join("test_other.bin");
        fs::write(&test_file, b"PK This is a test").expect("Failed to write test file");
        let is_binary = is_windows_binary(&test_file).expect("Error checking binary type");
        assert!(!is_binary);
    }

     #[test]
     fn test_is_windows_binary_empty_file() {
         let temp_dir = tempdir().expect("Failed to create temp dir");
         let test_file = temp_dir.path().join("test_empty.bin");
         fs::write(&test_file, b"").expect("Failed to write test file");
         let is_binary = is_windows_binary(&test_file).expect("Error checking binary type");
         assert!(!is_binary);
     }

     #[test]
     fn test_is_windows_binary_nonexistent_file() {
         let temp_dir = tempdir().expect("Failed to create temp dir");
         let non_existent_file = temp_dir.path().join("non_existent.bin");
         // is_windows_binary should return Ok(false) for a non-existent file
         let is_binary = is_windows_binary(&non_existent_file).expect("Error checking binary type for non-existent file");
         assert!(!is_binary);
     }

    // Note: Testing find_proton_path is difficult without a controlled environment
    // or mocking the file system and environment variables.

    // Note: Testing prepare_command_with_proton requires setting up a test environment
    // with a dummy 'proton' executable and checking the generated command.
    // This would be an integration test.
    #[test]
    fn test_prepare_command_with_proton() {
        let game_path = PathBuf::from("/path/to/game/game.exe");
        let proton_path = PathBuf::from("/fake/proton");
        let instance_index = 1;
        let base_wineprefix_dir = PathBuf::from("/tmp/test_wineprefixes");

        // Create a dummy directory for WINEPREFIX
        let instance_wineprefix = base_wineprefix_dir.join(format!("instance_{}_wineprefix", instance_index));
        std::fs::create_dir_all(&instance_wineprefix).expect("Failed to create dummy WINEPREFIX dir");


        let command_result = prepare_command_with_proton(
            &game_path,
            &proton_path,
            instance_index,
            &base_wineprefix_dir,
        );

        assert!(command_result.is_ok());
        let command = command_result.unwrap();

        // Check the command parts
        assert_eq!(command.get_program(), &*proton_path);
        let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
        assert!(args.contains(&std::ffi::OsStr::new("run")));
        assert!(args.contains(&*game_path.as_os_str()));

        // Check environment variables
        let envs: HashMap<std::ffi::OsString, std::ffi::OsString> = command.get_envs().filter_map(|(key, value_option)| {
             value_option.map(|value| (key.to_os_string(), value.to_os_string()))
        }).collect();

        assert_eq!(envs.get(&std::ffi::OsString::from("WINEPREFIX")).map(|s| s.to_string_lossy().to_string()), Some(instance_wineprefix.to_string_lossy().to_string()));
        assert_eq!(envs.get(&std::ffi::OsString::from("PROTON_LOG")).map(|s| s.to_string_lossy().to_string()), Some("1".to_string()));

        // Clean up dummy WINEPREFIX directory
        std::fs::remove_dir_all(&base_wineprefix_dir).expect("Failed to clean up dummy WINEPREFIX dir");

    }
}
