use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt};
use x11rb::rust_connection::RustConnection;
use std::error::Error;
use log::{info, error, warn, debug}; // Import debug
use std::sync::Arc;
use x11rb::errors::ReplyError;
use std::time::{Duration, Instant}; // Import Instant
use std::thread;
use std::collections::{HashMap, HashSet}; // Import HashMap and HashSet

// Custom error type for window management operations
#[derive(Debug)]
pub enum WindowManagerError {
    X11rbError(x11rb::errors::ConnectionError),
    X11rbReplyError(ReplyError),
    PropertyNotFound(xproto::Window, xproto::Atom),
    InvalidPropertyData(xproto::Window, xproto::Atom),
    MonitorDetectionError(String),
    WindowNotFound(Vec<u32>), // Include the PIDs that were not found
    GenericError(String),
}

impl std::fmt::Display for WindowManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WindowManagerError::X11rbError(e) => write!(f, "X11 connection error: {}", e),
            WindowManagerError::X11rbReplyError(e) => write!(f, "X11 reply error: {}", e),
            WindowManagerError::PropertyNotFound(window, atom) => {
                write!(f, "Property not found for window {}: {:?}", window, atom)
            }
            WindowManagerError::InvalidPropertyData(window, atom) => {
                write!(f, "Invalid property data for window {}: {:?}", window, atom)
            }
            WindowManagerError::MonitorDetectionError(msg) => write!(f, "Monitor detection error: {}", msg),
            WindowManagerError::WindowNotFound(pids) => {
                write!(f, "Window not found for PIDs: {:?}", pids)
            },
            WindowManagerError::GenericError(msg) => write!(f, "Window manager error: {}", msg),
        }
    }
}

impl Error for WindowManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WindowManagerError::X11rbError(e) => Some(e),
            WindowManagerError::X11rbReplyError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<x11rb::errors::ConnectionError> for WindowManagerError {
    fn from(err: x11rb::errors::ConnectionError) -> Self {
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

        let pid_atom_request = self.conn.intern_atom(false, "_NET_WM_PID");
        let windows_request = self.conn.query_tree(screen.root);

        let pid_atom = pid_atom_request?.reply()?.atom;
        let windows = windows_request?.reply()?.children;

        for window in windows {
            // Use `get_property_reply` to avoid blocking the loop unnecessarily if a property request fails
            let pid_prop_reply = self.conn.get_property(false, window, pid_atom, xproto::ATOM_CARDINAL, 0, 1)?.reply()?;
            if let Some(pid_prop_value) = pid_prop_reply.value {
                // _NET_WM_PID is a CARDINAL (u32)
                if pid_prop_value.len() == 4 { // Check if the property value has the expected size for a u32
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

    /// Finds a window by its _WM_NAME property (window title).
    /// Less reliable than finding by PID.
    /// Returns Ok(Some(window)) if found, Ok(None) if not found, and Err on X11 error.
    pub fn find_window_by_title(&self, title: &str) -> Result<Option<xproto::Window>, WindowManagerError> {
         debug!("Attempting to find window with title: {}", title);
        let setup = self.conn.setup();
        let screen = &setup.roots[0];
        let windows = self.conn.query_tree(screen.root)?.reply()?.children;

        for window in windows {
            let name_reply = self.conn.get_property(false, window, xproto::ATOM_WM_NAME, xproto::ATOM_STRING, 0, 1024)?.reply()?;
            if let Some(name_value) = name_reply.value {
                if let Ok(name_str) = String::from_utf8(name_value) {
                     debug!("Found window {} with title: {}", window, name_str.trim());
                    if name_str.trim() == title {
                        info!("Matched window {} with target title: {}", window, title);
                        return Ok(Some(window));
                    }
                }
            }
        }

         debug!("No window found with title: {}", title);
        Ok(None)
    }


    pub fn resize_window(&self, window: xproto::Window, width: u32, height: u32) -> Result<(), WindowManagerError> {
        info!("Resizing window {} to {}x{}", window, width, height);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::Width(width),
            xproto::ConfigWindow::Height(height),
        ])?.check()?; // Use check() to ensure the request was successful
         // No flush here, defer to the end of set_layout for batching
        Ok(())
    }

    pub fn move_window(&self, window: xproto::Window, x: i32, y: i32) -> Result<(), WindowManagerError> {
        info!("Moving window {} to ({}, {})", window, x, y);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::X(x),
            xproto::ConfigWindow::Y(y),
        ])?.check()?; // Use check() to ensure the request was successful
         // No flush here, defer to the end of set_layout for batching
        Ok(())
    }

    /// Attempts to remove window decorations using _MOTIF_WM_HINTS.
    /// Note: This method is older and might not work with all modern window managers/compositors.
    /// More robust decoration removal often involves setting EWMH properties like _NET_WM_STATE
    /// or influencing the window type, or potentially sending client messages.
    pub fn remove_decorations(&self, window: xproto::Window) -> Result<(), WindowManagerError> {
        info!("Attempting to remove decorations from window {}", window);
        let atom = self.conn.intern_atom(false, "_MOTIF_WM_HINTS")?.reply()?.atom;

        // _MOTIF_WM_HINTS format (from Motif Window Manager Hints):
        // flags       (32-bit)
        // functions   (32-bit)
        // decorations (32-bit)
        // input_mode  (32-bit)
        // status      (32-bit)
        // We set decorations to 0 (MWM_DECOR_NONE)
        let mut data = vec![0u32; 5];
        let MWM_HINTS_DECORATIONS = 1 << 1; // Flag to indicate decorations field is set
        data[0] = MWM_HINTS_DECORATIONS;
        data[2] = 0; // MWM_DECOR_NONE

        // The property value needs to be in bytes, CARDINAL format (32-bit unsigned integer)
        let data_bytes: Vec<u8> = data.iter()
            .flat_map(|&val| val.to_ne_bytes().into_iter())
            .collect();


        self.conn.change_property(
            xproto::PropMode::Replace,
            window,
            atom,
            xproto::ATOM_CARDINAL,
            32, // Format: 32-bit
            &data_bytes,
        )?.check()?;
         // No flush here, defer to the end of set_layout for batching
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
             for pid in unfound_pids.drain(..).collect::<Vec<_>>() {
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
         let mut ordered_windows: Vec<(u32, xproto::Window)> = window_pids.iter()
             .filter_map(|&pid| found_windows.get(&pid).map(|&window| (pid, window)))
             .collect();

         // The filter_map preserves the order of window_pids

         let num_windows = ordered_windows.len();
         let num_monitors = monitors.len();

         // Calculate layout parameters within the assigned monitor
         // This logic needs to be more sophisticated for complex layouts and monitor setups.
         // For simplicity, we distribute windows round-robin across monitors
         // and tile them within each monitor based on the layout.

         for (window_index, (pid, window_id)) in ordered_windows.iter().enumerate() {
             let monitor_index = window_index % num_monitors;
             let monitor = &monitors[monitor_index];

             // Simple tiling logic within the assigned monitor
             let (x, y, width, height) = match layout {
                 Layout::Horizontal => {
                     let num_windows_on_this_monitor = num_windows / num_monitors + (if monitor_index < num_windows % num_monitors { 1 } else { 0 });
                     let index_on_monitor = window_index / num_monitors; // Incorrect index calculation for horizontal
                     // Corrected index calculation for horizontal tiling within a monitor
                     let index_on_monitor = ordered_windows.iter().take(window_index)
                         .filter(|(_, &w)| {
                             let monitor_idx_for_w = ordered_windows.iter().position(|&(_, inner_w)| inner_w == w).unwrap() % num_monitors;
                             monitor_idx_for_w == monitor_index
                         })
                         .count();


                     let single_window_width = monitor.width / num_windows_on_this_monitor as i32;
                     let x_offset = index_on_monitor as i32 * single_window_width;
                     (monitor.x + x_offset, monitor.y, single_window_width, monitor.height)
                 }
                 Layout::Vertical => {
                     let num_windows_on_this_monitor = num_windows / num_monitors + (if monitor_index < num_windows % num_monitors { 1 } else { 0 });
                     // Corrected index calculation for vertical tiling within a monitor
                      let index_on_monitor = ordered_windows.iter().take(window_index)
                         .filter(|(_, &w)| {
                             let monitor_idx_for_w = ordered_windows.iter().position(|&(_, inner_w)| inner_w == w).unwrap() % num_monitors;
                             monitor_idx_for_w == monitor_index
                         })
                         .count();

                     let single_window_height = monitor.height / num_windows_on_this_monitor as i32;
                     let y_offset = index_on_monitor as i32 * single_window_height;
                     (monitor.x, monitor.y + y_offset, monitor.width, single_window_height)
                 }
             };

             info!("Applying layout for window {} (PID {}): monitor index {}, x={}, y={}, width={}, height={}", window_id, pid, monitor_index, x, y, width, height);

             // Apply transformations
             self.move_window(*window_id, x, y)?;
             self.resize_window(*window_id, width as u32, height as u32)?;
             self.remove_decorations(*window_id)?; // Optional: Remove decorations
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
         let atom = self.conn.intern_atom(false, "_NET_WORKAREA")?.reply()?.atom;
         let reply = self.conn.get_property(false, root, atom, xproto::ATOM_CARDINAL, 0, u32::MAX)?.reply()?; // Get the full property value

         if let Some(value) = reply.value {
             // _NET_WORKAREA is a list of CARDINALs (u32) in groups of 4: x, y, width, height
             if value.len() % 16 != 0 || value.is_empty() { // 4 u32s = 16 bytes
                 error!("_NET_WORKAREA property has unexpected size or is empty: {} bytes. Expected a non-zero multiple of 16.", value.len());
                  return Err(WindowManagerError::InvalidPropertyData(root, atom));
             }

             let mut monitors = Vec::new();
             // Process the bytes in chunks of 16 (4 u32s)
             for (i, chunk) in value.chunks_exact(16).enumerate() {
                 let x = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i32;
                 let y = u32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as i32;
                 let width = u32::from_ne_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]) as i32;
                 let height = u32::from_ne_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]) as i32;
                 monitors.push(Monitor { x, y, width, height });
                  info!("Detected monitor {}: x={}, y={}, width={}, height={}", i, x, y, width, height);
             }
              info!("Detected {} monitors based on _NET_WORKAREA.", monitors.len());
             return Ok(monitors);
         }

         // If the property is not found or empty (value is None)
          error!("_NET_WORKAREA property not found or is empty (value is None).");
         Err(WindowManagerError::MonitorDetectionError("_NET_WORKAREA property not available".to_string()))
     }
}

#[derive(Debug)] // Derive Debug for Layout enum
pub enum Layout {
    Horizontal,
    Vertical,
    // Consider adding more layouts like Grid
}

impl From<&str> for Layout {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "vertical" => Layout::Vertical,
            "horizontal" => Layout::Horizontal,
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
