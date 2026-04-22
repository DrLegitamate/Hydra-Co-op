use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, AtomEnum, ConfigureWindowAux, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::errors::{ConnectError, ConnectionError, ReplyError};
use std::error::Error;
use log::{info, error, warn, debug};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;
use std::collections::{HashMap, HashSet};

// Custom error type for window management operations
#[derive(Debug)]
pub enum WindowManagerError {
    X11rbConnectError(ConnectError),
    X11rbError(ConnectionError),
    X11rbReplyError(ReplyError),
    InvalidPropertyData(xproto::Window, xproto::Atom),
    MonitorDetectionError(String),
    WindowNotFound(Vec<u32>),
}

impl std::fmt::Display for WindowManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WindowManagerError::X11rbConnectError(e) => write!(f, "X11 connect error: {}", e),
            WindowManagerError::X11rbError(e) => write!(f, "X11 connection error: {}", e),
            WindowManagerError::X11rbReplyError(e) => write!(f, "X11 reply error: {}", e),
            WindowManagerError::InvalidPropertyData(window, atom) => {
                write!(f, "Invalid property data for window {}: {:?}", window, atom)
            }
            WindowManagerError::MonitorDetectionError(msg) => write!(f, "Monitor detection error: {}", msg),
            WindowManagerError::WindowNotFound(pids) => {
                write!(f, "Window not found for PIDs: {:?}", pids)
            },
        }
    }
}

impl Error for WindowManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WindowManagerError::X11rbConnectError(e) => Some(e),
            WindowManagerError::X11rbError(e) => Some(e),
            WindowManagerError::X11rbReplyError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ConnectError> for WindowManagerError {
    fn from(err: ConnectError) -> Self {
        WindowManagerError::X11rbConnectError(err)
    }
}

impl From<ConnectionError> for WindowManagerError {
    fn from(err: ConnectionError) -> Self {
        WindowManagerError::X11rbError(err)
    }
}

impl From<ReplyError> for WindowManagerError {
    fn from(err: ReplyError) -> Self {
        WindowManagerError::X11rbReplyError(err)
    }
}


pub struct WindowManager {
    conn: Arc<RustConnection>,
}

impl WindowManager {
    pub fn new() -> Result<Self, WindowManagerError> {
        let (conn, _) = RustConnection::connect(None)?;
        Ok(WindowManager { conn: Arc::new(conn) })
    }

    /// Finds a window by its _NET_WM_PID property.
    /// This is generally more reliable than finding by title.
    /// Returns Ok(Some(window)) if found, Ok(None) if not found, and Err on X11 error.
    pub fn find_window_by_pid(&self, pid: u32) -> Result<Option<xproto::Window>, WindowManagerError> {
        debug!("Attempting to find window with PID: {}", pid);
        let setup = self.conn.setup();
        let screen = &setup.roots[0];

        let pid_atom_request = self.conn.intern_atom(false, b"_NET_WM_PID");
        let windows_request = self.conn.query_tree(screen.root);

        let pid_atom = pid_atom_request?.reply()?.atom;
        let windows = windows_request?.reply()?.children;

        for window in windows {
            let pid_prop_reply = self.conn.get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)?.reply()?;
            let pid_prop_value = &pid_prop_reply.value;
            if !pid_prop_value.is_empty() {
                if pid_prop_value.len() == 4 {
                    let window_pid = u32::from_ne_bytes([
                        pid_prop_value[0],
                        pid_prop_value[1],
                        pid_prop_value[2],
                        pid_prop_value[3],
                    ]);
                    debug!("Found window {} with PID {}", window, window_pid);
                    if window_pid == pid {
                        info!("Matched window {} with target PID {}", window, pid);
                        return Ok(Some(window));
                    }
                } else {
                    debug!("Window {} has _NET_WM_PID property with unexpected size: {}", window, pid_prop_value.len());
                }
            }
        }

        debug!("No window found with PID: {}", pid);
        Ok(None)
    }

    pub fn resize_window(&self, window: xproto::Window, width: u32, height: u32) -> Result<(), WindowManagerError> {
        info!("Resizing window {} to {}x{}", window, width, height);
        let aux = ConfigureWindowAux::new().width(width).height(height);
        self.conn.configure_window(window, &aux)?.check()?;
        Ok(())
    }

    pub fn move_window(&self, window: xproto::Window, x: i32, y: i32) -> Result<(), WindowManagerError> {
        info!("Moving window {} to ({}, {})", window, x, y);
        let aux = ConfigureWindowAux::new().x(x).y(y);
        self.conn.configure_window(window, &aux)?.check()?;
        Ok(())
    }

    /// Attempts to remove window decorations using _MOTIF_WM_HINTS.
    /// Note: This method is older and might not work with all modern window managers/compositors.
    /// More robust decoration removal often involves setting EWMH properties like _NET_WM_STATE
    /// or influencing the window type, or potentially sending client messages.
    pub fn remove_decorations(&self, window: xproto::Window) -> Result<(), WindowManagerError> {
        info!("Attempting to remove decorations from window {}", window);
        let atom = self.conn.intern_atom(false, b"_MOTIF_WM_HINTS")?.reply()?.atom;

        // _MOTIF_WM_HINTS layout: flags, functions, decorations, input_mode, status (5 x u32).
        const MWM_HINTS_DECORATIONS: u32 = 1 << 1;
        let data: [u32; 5] = [MWM_HINTS_DECORATIONS, 0, 0, 0, 0];
        let data_bytes: Vec<u8> = data.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        self.conn.change_property(
            PropMode::REPLACE,
            window,
            atom,
            AtomEnum::CARDINAL,
            32,
            data.len() as u32,
            &data_bytes,
        )?.check()?;
        info!("Sent request to remove decorations for window {}", window);
        Ok(())
    }


     /// Sets the layout of the given windows on the screen(s).
     /// This function attempts to find the windows by their PIDs with retries
     /// and exponential backoff. Once found, it applies the specified layout.
     ///
     /// Note: This is a basic implementation. For robust multi-monitor support,
     /// you would need a more sophisticated algorithm to assign windows to specific
     /// monitor areas and calculate their positions and sizes accordingly.
     ///
     /// # Arguments
     ///
     /// * `window_pids` - A slice of process IDs for the windows to manage. The order
     ///                   in this slice determines the order in which windows are
     ///                   assigned positions in the layout.
     /// * `layout` - The desired layout (Horizontal, Vertical).
     ///
     /// # Returns
     ///
     /// * `Result<(), WindowManagerError>` - Ok(()) on success, Err on failure to find
     ///                                      windows or apply layout.
     pub fn set_layout(&self, window_pids: &[u32], layout: Layout) -> Result<(), WindowManagerError> {
         info!("Starting to set layout {:?} for windows with PIDs: {:?}", layout, window_pids);

         if window_pids.is_empty() {
             warn!("No window PIDs provided for layout.");
             return Ok(()); // Nothing to do if no PIDs are given
         }

         let monitors = self.get_monitors()?;

         if monitors.is_empty() {
             error!("No monitors detected. Cannot set window layout.");
              return Err(WindowManagerError::MonitorDetectionError("No monitors found".to_string()));
         }

         let mut found_windows: HashMap<u32, xproto::Window> = HashMap::new();
         let mut unfound_pids: HashSet<u32> = window_pids.iter().cloned().collect();

         let start_time = Instant::now();
         let max_wait_duration = Duration::from_secs(30); // Maximum time to wait for windows (e.g., 30 seconds)
         let mut current_delay = Duration::from_millis(50); // Initial delay for exponential backoff
         let max_delay = Duration::from_millis(500); // Maximum delay between retries

         info!("Attempting to find {} windows with a maximum wait of {:?}.", window_pids.len(), max_wait_duration);

         // Main loop to find windows with exponential backoff
         while !unfound_pids.is_empty() && start_time.elapsed() < max_wait_duration {
             debug!("Searching for {} unfound windows...", unfound_pids.len());
             let mut found_in_this_pass = Vec::new(); // PIDs found in the current iteration

             // Iterate over a drained list to avoid modifying the set while iterating
             for pid in unfound_pids.drain().collect::<Vec<_>>() {
                 match self.find_window_by_pid(pid) {
                     Ok(Some(window_id)) => {
                         info!("Successfully found window {} for PID {}", window_id, pid);
                         found_windows.insert(pid, window_id);
                         found_in_this_pass.push(pid);
                     }
                     Ok(None) => {
                         debug!("Window for PID {} not found in this pass.", pid);
                         // Re-insert into unfound_pids for the next iteration
                         unfound_pids.insert(pid);
                     }
                     Err(e) => {
                         error!("Error while searching for window for PID {}: {}", pid, e);
                         // Decide how to handle this error during the search.
                         // For now, let's propagate it.
                         return Err(e);
                     }
                 }
             }

             if !unfound_pids.is_empty() {
                 info!("{} windows still unfound. Waiting {:?} before retrying...", unfound_pids.len(), current_delay);
                 thread::sleep(current_delay);
                 current_delay = std::cmp::min(current_delay * 2, max_delay); // Exponential backoff
             } else {
                 info!("All windows found.");
             }
         }

         // After the waiting loop, check if all windows were found
         if !unfound_pids.is_empty() {
             error!("Failed to find all windows after waiting {:?}. Unfound PIDs: {:?}", start_time.elapsed(), unfound_pids);
             return Err(WindowManagerError::WindowNotFound(unfound_pids.into_iter().collect()));
         }

         info!("All required windows found. Proceeding with layout application.");

         // Now apply the layout using the found window IDs.
         // Ensure the order matches the original window_pids slice.
         let ordered_windows: Vec<(u32, xproto::Window)> = window_pids.iter()
             .filter_map(|&pid| found_windows.get(&pid).map(|&window| (pid, window)))
             .collect();

         let num_windows = ordered_windows.len();
         let num_monitors = monitors.len();

         // Round-robin windows across monitors, then tile within each monitor.
         for (window_index, (pid, window_id)) in ordered_windows.iter().enumerate() {
             let monitor_index = window_index % num_monitors;
             let monitor = &monitors[monitor_index];
             // index_on_monitor: 0-based slot for this window within its assigned monitor.
             let index_on_monitor = window_index / num_monitors;
             // Total windows assigned to this monitor under round-robin distribution.
             let windows_on_monitor = (num_windows + num_monitors - 1 - monitor_index) / num_monitors;

             let (x, y, width, height): (i32, i32, u32, u32) = match &layout {
                 Layout::Horizontal => {
                     let single_width = monitor.width / windows_on_monitor.max(1) as i32;
                     let x_offset = index_on_monitor as i32 * single_width;
                     (monitor.x + x_offset, monitor.y, single_width as u32, monitor.height as u32)
                 }
                 Layout::Vertical => {
                     let single_height = monitor.height / windows_on_monitor.max(1) as i32;
                     let y_offset = index_on_monitor as i32 * single_height;
                     (monitor.x, monitor.y + y_offset, monitor.width as u32, single_height as u32)
                 }
                 Layout::Grid2x2 => {
                     let grid_x = window_index % 2;
                     let grid_y = (window_index / 2) % 2;
                     let cell_width = monitor.width / 2;
                     let cell_height = monitor.height / 2;
                     let x = monitor.x + (grid_x as i32 * cell_width);
                     let y = monitor.y + (grid_y as i32 * cell_height);
                     (x, y, cell_width as u32, cell_height as u32)
                 }
                 Layout::Grid3x1 => {
                     let cell_width = monitor.width / 3;
                     let x = monitor.x + ((window_index % 3) as i32 * cell_width);
                     (x, monitor.y, cell_width as u32, monitor.height as u32)
                 }
             };

             info!("Applying layout for window {} (PID {}): monitor index {}, x={}, y={}, width={}, height={}", window_id, pid, monitor_index, x, y, width, height);

             self.move_window(*window_id, x, y)?;
             self.resize_window(*window_id, width, height)?;
             self.remove_decorations(*window_id)?;
         }

         self.conn.flush()?; // Ensure all requests are sent after all operations
         info!("Window layout set successfully.");
         Ok(())
     }

     /// Retrieves monitor information using the _NET_WORKAREA EWMH property.
     /// Returns a list of usable desktop areas.
     /// This is generally more reliable than SCREEN information as it respects panels/docks.
     fn get_monitors(&self) -> Result<Vec<Monitor>, WindowManagerError> {
         info!("Attempting to get monitor information using _NET_WORKAREA");
         let root = self.conn.setup().roots[0].root;
         let atom = self.conn.intern_atom(false, b"_NET_WORKAREA")?.reply()?.atom;
         let reply = self.conn.get_property(false, root, atom, AtomEnum::CARDINAL, 0, u32::MAX)?.reply()?;
         let value = reply.value;

         if value.is_empty() {
             error!("_NET_WORKAREA property not found or is empty.");
             return Err(WindowManagerError::MonitorDetectionError("_NET_WORKAREA property not available".to_string()));
         }
         if value.len() % 16 != 0 {
             error!("_NET_WORKAREA property has unexpected size: {} bytes. Expected a multiple of 16.", value.len());
             return Err(WindowManagerError::InvalidPropertyData(root, atom));
         }

         let mut monitors = Vec::new();
         for (i, chunk) in value.chunks_exact(16).enumerate() {
             let x = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i32;
             let y = u32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as i32;
             let width = u32::from_ne_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]) as i32;
             let height = u32::from_ne_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]) as i32;
             monitors.push(Monitor { x, y, width, height });
             info!("Detected monitor {}: x={}, y={}, width={}, height={}", i, x, y, width, height);
         }
         info!("Detected {} monitors based on _NET_WORKAREA.", monitors.len());
         Ok(monitors)
     }
}

#[derive(Debug)]
pub enum Layout {
    Horizontal,
    Vertical,
    Grid2x2,
    Grid3x1,
}

impl From<&str> for Layout {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "vertical" => Layout::Vertical,
            "horizontal" => Layout::Horizontal,
            "grid2x2" => Layout::Grid2x2,
            "grid3x1" => Layout::Grid3x1,
            _ => {
                log::warn!("Unknown layout '{}', defaulting to Horizontal.", s);
                Layout::Horizontal // Default layout
            }
        }
    }
}

#[derive(Debug)] // Derive Debug for Monitor struct
struct Monitor {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

// Add tests similar to instance_manager.rs if possible,
// but X11 interaction makes these harder without a virtual display.
// You might need integration tests that run in an X11 environment.

#[cfg(test)]
mod tests {
    // Mock X11 server interaction is complex.
    // These tests would primarily verify the logic *given* successful X11 calls.
    // Real-world testing requires an X server.

    // Example test structure (would require mocking x11rb responses)
    // #[test]
    // fn test_set_layout_finds_windows_with_retry() {
    //     // Mock a WindowManager that initially doesn't find a PID, then finds it on retry
    // }

    // #[test]
    // fn test_set_layout_fails_if_windows_not_found() {
    //     // Mock a WindowManager that never finds a specific PID
    // }

    // #[test]
    // fn test_set_layout_applies_correct_positions_horizontal() {
    //     // Mock get_monitors to return specific monitor sizes
    //     // Mock find_window_by_pid to return window IDs
    //     // Verify move_window and resize_window are called with expected arguments
    // }

     // #[test]
    // fn test_set_layout_applies_correct_positions_vertical() {
    //     // Similar to horizontal test, but verify vertical tiling
    // }
}
