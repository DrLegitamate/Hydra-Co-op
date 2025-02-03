use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::str;

/// Checks if the given file is a Windows PE (Portable Executable) binary.
fn is_windows_binary(file_path: &str) -> bool {
    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut buffer = [0; 2];
    if file.read_exact(&mut buffer).is_err() {
        return false;
    }

    buffer == [0x4D, 0x5A] // "MZ" signature for PE files
}

/// Verifies that Proton is installed on the system and selects the appropriate version.
fn get_proton_path() -> Option<String> {
    // Allow the user to specify the Proton path via an environment variable
    if let Ok(proton_path) = env::var("PROTON_PATH") {
        let path = Path::new(&proton_path);
        if path.exists() {
            return Some(proton_path);
        }
    }

    // Default path for Proton installation; adjust as necessary
    let default_proton_path = Path::new("/usr/bin/proton");
    if default_proton_path.exists() {
        return Some(default_proton_path.to_string_lossy().into_owned());
    }

    None
}

/// Sets the necessary environment variables and invokes the game with Proton.
fn launch_game_with_proton(game_path: &str, proton_path: &str) -> io::Result<std::process::Child> {
    let mut command = Command::new(proton_path);
    command.arg(game_path);

    // Set environment variables if necessary
    command.env("PROTON_LOG", "1"); // Enable Proton logging

    // Spawn the process
    let child = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    Ok(child)
}

/// Main function to handle launching a Windows game via Proton.
pub fn launch_game(game_path: &str) -> io::Result<()> {
    if !is_windows_binary(game_path) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("The specified file '{}' is not a Windows binary.", game_path),
        ));
    }

    let proton_path = match get_proton_path() {
        Some(path) => path,
        None => return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Proton is not installed on the system.",
        )),
    };

    match launch_game_with_proton(game_path, &proton_path) {
        Ok(child) => {
            println!("Game launched successfully with PID: {}", child.id());
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to launch game: {}", e);
            Err(e)
        }
    }
}
