use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt};
use x11rb::rust_connection::RustConnection;
use std::error::Error;
use log::info;
use std::sync::Arc;
use x11rb::errors::ReplyError;
use std::time::Duration;
use std::thread;

// Custom error type for window management operations
#[derive(Debug)]
pub enum WindowManagerError {
    X11rbError(x11rb::errors::ConnectionError),
    X11rbReplyError(ReplyError),
    PropertyNotFound(xproto::Window, xproto::Atom),
    InvalidPropertyData(xproto::Window, xproto::Atom),
    MonitorDetectionError(String),
    WindowNotFound,
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
            WindowManagerError::WindowNotFound => write!(f, "Window not found"),
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
    pub fn find_window_by_pid(&self, pid: u32) -> Result<Option<xproto::Window>, WindowManagerError> {
        info!("Attempting to find window with PID: {}", pid);
        let setup = self.conn.setup();
        let screen = &setup.roots[0];
        let windows = self.conn.query_tree(screen.root)?.reply()?.children;

        let pid_atom = self.conn.intern_atom(false, "_NET_WM_PID")?.reply()?.atom;

        for window in windows {
            let pid_prop = self.conn.get_property(false, window, pid_atom, xproto::ATOM_CARDINAL, 0, 1)?.reply()?;
            if let Some(pid_prop_value) = pid_prop.value {
                // _NET_WM_PID is a CARDINAL (u32)
                if pid_prop_value.len() == 4 { // Check if the property value has the expected size for a u32
                    let window_pid = u32::from_ne_bytes([
                        pid_prop_value[0],
                        pid_prop_value[1],
                        pid_prop_value[2],
                        pid_prop_value[3],
                    ]);
                     info!("Found window {} with PID {}", window, window_pid);
                    if window_pid == pid {
                        info!("Matched window {} with target PID {}", window, pid);
                        return Ok(Some(window));
                    }
                } else {
                     info!("Window {} has _NET_WM_PID property with unexpected size: {}", window, pid_prop_value.len());
                }
            }
        }

        info!("No window found with PID: {}", pid);
        Ok(None)
    }

    /// Finds a window by its _WM_NAME property (window title).
    /// Less reliable than finding by PID.
    pub fn find_window_by_title(&self, title: &str) -> Result<Option<xproto::Window>, WindowManagerError> {
         info!("Attempting to find window with title: {}", title);
        let setup = self.conn.setup();
        let screen = &setup.roots[0];
        let windows = self.conn.query_tree(screen.root)?.reply()?.children;

        for window in windows {
            let name = self.conn.get_property(false, window, xproto::ATOM_WM_NAME, xproto::ATOM_STRING, 0, 1024)?.reply()?;
            if let Some(name_value) = name.value {
                if let Ok(name_str) = String::from_utf8(name_value) {
                     info!("Found window {} with title: {}", window, name_str.trim());
                    if name_str.trim() == title {
                        info!("Matched window {} with target title: {}", window, title);
                        return Ok(Some(window));
                    }
                }
            }
        }

        info!("No window found with title: {}", title);
        Ok(None)
    }


    pub fn resize_window(&self, window: xproto::Window, width: u32, height: u32) -> Result<(), WindowManagerError> {
        info!("Resizing window {} to {}x{}", window, width, height);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::Width(width),
            xproto::ConfigWindow::Height(height),
        ])?.check()?; // Use check() to ensure the request was successful
        self.conn.flush()?; // Ensure the request is sent
        Ok(())
    }

    pub fn move_window(&self, window: xproto::Window, x: i32, y: i32) -> Result<(), WindowManagerError> {
        info!("Moving window {} to ({}, {})", window, x, y);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::X(x),
            xproto::ConfigWindow::Y(y),
        ])?.check()?; // Use check() to ensure the request was successful
         self.conn.flush()?; // Ensure the request is sent
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
        // flags      (32-bit)
        // functions  (32-bit)
        // decorations(32-bit)
        // input_mode (32-bit)
        // status     (32-bit)
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
        self.conn.flush()?;
        info!("Sent request to remove decorations for window {}", window);
        Ok(())
    }


     /// Sets the layout of the given windows on the screen(s).
     ///
     /// Note: This is a basic implementation. For robust multi-monitor support,
     /// you would need a more sophisticated algorithm to assign windows to specific
     /// monitor areas and calculate their positions and sizes accordingly.
     pub fn set_layout(&self, window_pids: &[u32], layout: Layout) -> Result<(), WindowManagerError> {
        info!("Setting layout {:?} for windows with PIDs: {:?}", layout, window_pids);
        let monitors = self.get_monitors()?;

        if monitors.is_empty() {
            error!("No monitors detected. Cannot set window layout.");
             // Return a more specific error or handle this case appropriately
             return Err(WindowManagerError::MonitorDetectionError("No monitors found".to_string()));
        }

        // For simplicity, this example distributes windows across available monitors
        // in a round-robin fashion and tiles them within each monitor.
        // A more advanced approach might consider primary monitors, specific monitor indices, etc.

        let mut window_index = 0;
        for pid in window_pids {
            // Find the window by PID. This might require waiting if the window
            // hasn't appeared yet after the process was launched.
            // A robust solution would poll or listen for window creation events.
            // This example uses a simple loop with a delay.
            let mut window = None;
            let max_retries = 10; // Example: Retry up to 10 times
            let retry_delay = Duration::from_millis(500); // Example: Wait 500ms between retries

            for i in 0..max_retries {
                info!("Attempting to find window for PID {} (Attempt {}/{})", pid, i + 1, max_retries);
                match self.find_window_by_pid(*pid) {
                    Ok(Some(found_window)) => {
                        window = Some(found_window);
                        info!("Found window {} for PID {}", found_window, pid);
                        break; // Window found, exit retry loop
                    }
                    Ok(None) => {
                        info!("Window for PID {} not found yet, waiting...", pid);
                        thread::sleep(retry_delay);
                    }
                    Err(e) => {
                         error!("Error finding window for PID {}: {}", pid, e);
                         return Err(e); // Propagate the error if finding fails
                    }
                }
            }

            let window_id = match window {
                Some(id) => id,
                None => {
                    error!("Failed to find window for PID {} after multiple retries.", pid);
                     // Decide how to handle this: skip the window, return an error, etc.
                     return Err(WindowManagerError::WindowNotFound); // Example: Return an error
                }
            };

            let monitor_index = window_index % monitors.len();
            let monitor = &monitors[monitor_index];

            // Calculate layout parameters within the assigned monitor
            let num_windows_on_this_monitor = window_pids.len() / monitors.len() + (if window_index < window_pids.len() % monitors.len() * (window_pids.len() / monitors.len() + 1) { 1 } else { 0 }); // Simplified logic for distribution

            let (x, y, width, height) = match layout {
                Layout::Horizontal => {
                    // Simple horizontal split within the monitor's work area
                    let single_window_width = monitor.width / num_windows_on_this_monitor as i32;
                    let x_offset = (window_index / monitors.len()) as i32 * single_window_width;
                    (monitor.x + x_offset, monitor.y, single_window_width, monitor.height)
                }
                Layout::Vertical => {
                    // Simple vertical split within the monitor's work area
                     let windows_on_prev_monitors = (window_index / monitors.len()) * (window_pids.len() / monitors.len() + (if window_index % monitors.len() > 0 { 1 } else { 0 })); // Rough calculation of windows on previous monitors
                     let instance_index_on_monitor = window_index % (window_pids.len() / monitors.len() + (if monitor_index < window_pids.len() % monitors.len() { 1 } else { 0 })); // Index of the window on this specific monitor

                     let single_window_height = monitor.height / num_windows_on_this_monitor as i32;
                     let y_offset = instance_index_on_monitor as i32 * single_window_height;

                    (monitor.x, monitor.y + y_offset, monitor.width, single_window_height)
                }
            };

            info!("Applying layout for window {}: x={}, y={}, width={}, height={}", window_id, x, y, width, height);

            // Apply transformations
            self.move_window(window_id, x, y)?;
            self.resize_window(window_id, width as u32, height as u32)?;
            self.remove_decorations(window_id)?; // Optional: Remove decorations

            window_index += 1;
        }

         self.conn.flush()?; // Ensure all requests are sent
        Ok(())
    }

     /// Retrieves monitor information using the _NET_WORKAREA EWMH property.
     /// Returns a list of usable desktop areas.
     fn get_monitors(&self) -> Result<Vec<Monitor>, WindowManagerError> {
         info!("Attempting to get monitor information using _NET_WORKAREA");
        let root = self.conn.setup().roots[0].root;
        let atom = self.conn.intern_atom(false, "_NET_WORKAREA")?.reply()?.atom;
        let reply = self.conn.get_property(false, root, atom, xproto::ATOM_CARDINAL, 0, u32::MAX)?.reply()?; // Get the full property value

        if let Some(value) = reply.value {
            // _NET_WORKAREA is a list of CARDINALs (u32) in groups of 4: x, y, width, height
            if value.len() % 16 != 0 { // 4 u32s = 16 bytes
                 error!("_NET_WORKAREA property has unexpected size: {} bytes. Expected a multiple of 16.", value.len());
                 // Depending on requirements, you might return an error or try to parse what's available
                 return Err(WindowManagerError::InvalidPropertyData(root, atom));
            }

            let mut monitors = Vec::new();
            // Process the bytes in chunks of 16 (4 u32s)
            for chunk in value.chunks_exact(16) {
                let x = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i32;
                let y = u32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as i32;
                let width = u32::from_ne_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]) as i32;
                let height = u32::from_ne_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]) as i32;
                monitors.push(Monitor { x, y, width, height });
                 info!("Detected monitor: x={}, y={}, width={}, height={}", x, y, width, height);
            }
             info!("Detected {} monitors.", monitors.len());
            return Ok(monitors);
        }

        // If the property is not found or empty
         error!("_NET_WORKAREA property not found or is empty.");
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
                // Default to Horizontal or return an error/panic depending on desired behavior
                // For a robust application, you might want to return a Result here.
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
