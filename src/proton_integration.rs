use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use log::{info, error, warn, debug};
use std::error::Error;

// Custom error type for Proton integration operations
#[derive(Debug)]
pub enum ProtonError {
    IoError(io::Error),
    ProtonNotFound(String),
}

impl std::fmt::Display for ProtonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ProtonError::IoError(e) => write!(f, "Proton integration I/O error: {}", e),
            ProtonError::ProtonNotFound(msg) => write!(f, "Proton not found: {}", msg),
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
pub fn is_windows_binary(file_path: &Path) -> Result<bool, ProtonError> {
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
///
/// Search order:
/// 1. `PROTON_PATH` environment variable.
/// 2. Common Steam installation paths (`~/.steam`, `~/.local/share/Steam`, Flatpak).
///    Any `Proton*/proton` binary found is returned (newest version first by name).
///
/// Returns the path to the `proton` script if found.
pub fn find_proton_path() -> Result<PathBuf, ProtonError> {
    info!("Attempting to find Proton executable.");

    // 1. Explicit override via environment variable.
    if let Ok(proton_path_env) = env::var("PROTON_PATH") {
        let path = PathBuf::from(&proton_path_env);
        if path.exists() {
            info!("Found Proton via PROTON_PATH: {}", path.display());
            return Ok(path);
        }
        warn!("PROTON_PATH='{}' does not exist — continuing search.", proton_path_env);
    }

    // 2. Search common Steam library locations.
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home"));

    let steam_roots: Vec<PathBuf> = vec![
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".local/share/Steam"),
        // Flatpak Steam
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        // Snap Steam
        home.join("snap/steam/common/.local/share/Steam"),
    ];

    for steam_root in &steam_roots {
        let steamapps = steam_root.join("steamapps/common");
        if !steamapps.is_dir() {
            continue;
        }
        debug!("Searching for Proton in {}", steamapps.display());

        // Collect all Proton* subdirectories, then sort descending so we get the
        // newest version first (e.g. "Proton 9.0" before "Proton 8.0").
        let mut proton_dirs: Vec<PathBuf> = fs::read_dir(&steamapps)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_dir()
                            && p.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.starts_with("Proton"))
                                .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();

        proton_dirs.sort_by(|a, b| b.cmp(a)); // descending — newest version first

        for dir in &proton_dirs {
            let exe = dir.join("proton");
            if exe.exists() {
                info!("Found Proton at: {}", exe.display());
                return Ok(exe);
            }
        }
    }

    // 3. Check additional Steam library folders listed in libraryfolders.vdf.
    for steam_root in &steam_roots {
        let vdf = steam_root.join("steamapps/libraryfolders.vdf");
        if let Ok(contents) = fs::read_to_string(&vdf) {
            for line in contents.lines() {
                // VDF lines look like:  "path"  "/mnt/games/SteamLibrary"
                if line.trim_start().starts_with("\"path\"") {
                    let path_str = line
                        .split('"')
                        .nth(3)
                        .unwrap_or("")
                        .replace("\\\\", "/");
                    let alt_steamapps = PathBuf::from(&path_str).join("steamapps/common");
                    if alt_steamapps.is_dir() {
                        let mut proton_dirs: Vec<PathBuf> = fs::read_dir(&alt_steamapps)
                            .map(|entries| {
                                entries
                                    .filter_map(|e| e.ok())
                                    .map(|e| e.path())
                                    .filter(|p| {
                                        p.is_dir()
                                            && p.file_name()
                                                .and_then(|n| n.to_str())
                                                .map(|n| n.starts_with("Proton"))
                                                .unwrap_or(false)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        proton_dirs.sort_by(|a, b| b.cmp(a));
                        for dir in &proton_dirs {
                            let exe = dir.join("proton");
                            if exe.exists() {
                                info!("Found Proton in extra library at: {}", exe.display());
                                return Ok(exe);
                            }
                        }
                    }
                }
            }
        }
    }

    error!("Proton executable not found in any known location.");
    Err(ProtonError::ProtonNotFound(
        "Proton not found. Install it via Steam (Library → Tools → 'Proton X.Y') \
         or set the PROTON_PATH environment variable to its location."
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
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
        let is_binary = is_windows_binary(&non_existent_file).expect("Error checking binary type for non-existent file");
        assert!(!is_binary);
    }
}
