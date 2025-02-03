use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt};
use x11rb::rust_connection::RustConnection;
use std::error::Error;
use log::info;
use std::env;
use std::sync::Arc;

pub struct WindowManager {
    conn: Arc<RustConnection>,
}

impl WindowManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let (conn, _) = RustConnection::connect(None)?;
        Ok(WindowManager { conn: Arc::new(conn) })
    }

    pub fn find_window_by_title(&self, title: &str) -> Result<Option<xproto::Window>, Box<dyn Error>> {
        let setup = self.conn.setup();
        let screen = &setup.roots[0];
        let windows = self.conn.query_tree(screen.root)?.reply()?.children;

        for window in windows {
            let name = self.conn.get_property(false, window, xproto::ATOM_WM_NAME, xproto::ATOM_STRING, 0, 1024)?.reply()?;
            if let Some(name) = name.value {
                if let Ok(name_str) = String::from_utf8(name) {
                    if name_str.trim() == title {
                        return Ok(Some(window));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn find_window_by_pid(&self, pid: u32) -> Result<Option<xproto::Window>, Box<dyn Error>> {
        let setup = self.conn.setup();
        let screen = &setup.roots[0];
        let windows = self.conn.query_tree(screen.root)?.reply()?.children;

        for window in windows {
            let pid_atom = self.conn.intern_atom(false, "_NET_WM_PID")?.reply()?.atom;
            let pid_prop = self.conn.get_property(false, window, pid_atom, xproto::ATOM_CARDINAL, 0, 1)?.reply()?;
            if let Some(pid_prop) = pid_prop.value {
                if let Some(pid_value) = pid_prop.first() {
                    if *pid_value as u32 == pid {
                        return Ok(Some(window));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn resize_window(&self, window: xproto::Window, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
        info!("Resizing window {} to {}x{}", window, width, height);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::Width(width),
            xproto::ConfigWindow::Height(height),
        ])?;
        Ok(())
    }

    pub fn move_window(&self, window: xproto::Window, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        info!("Moving window {} to ({}, {})", window, x, y);
        self.conn.configure_window(window, &[
            xproto::ConfigWindow::X(x),
            xproto::ConfigWindow::Y(y),
        ])?;
        Ok(())
    }

    pub fn remove_decorations(&self, window: xproto::Window) -> Result<(), Box<dyn Error>> {
        info!("Removing decorations from window {}", window);
        let atom = self.conn.intern_atom(false, "_MOTIF_WM_HINTS")?.reply()?.atom;
        let data = vec![0u8; 9];
        self.conn.change_property(xproto::PropMode::Replace, window, atom, xproto::ATOM_CARDINAL, 32, &data)?;
        Ok(())
    }

    pub fn set_layout(&self, windows: Vec<xproto::Window>, layout: Layout) -> Result<(), Box<dyn Error>> {
        let monitors = self.get_monitors()?;
        let mut monitor_index = 0;

        for window in windows {
            let monitor = &monitors[monitor_index % monitors.len()];
            let (x, y, width, height) = match layout {
                Layout::Horizontal => (monitor.x, monitor.y, monitor.width / windows.len() as i32, monitor.height),
                Layout::Vertical => (monitor.x, monitor.y + (monitor.height / windows.len() as i32) * monitor_index, monitor.width, monitor.height / windows.len() as i32),
            };

            self.move_window(window, x, y)?;
            self.resize_window(window, width as u32, height as u32)?;
            self.remove_decorations(window)?;

            monitor_index += 1;
        }

        Ok(())
    }

    fn get_monitors(&self) -> Result<Vec<Monitor>, Box<dyn Error>> {
        let root = self.conn.setup().roots[0].root;
        let atom = self.conn.intern_atom(false, "_NET_WORKAREA")?.reply()?.atom;
        let reply = self.conn.get_property(false, root, atom, xproto::ATOM_CARDINAL, 0, 4)?.reply()?;

        if let Some(value) = reply.value {
            let mut monitors = Vec::new();
            for chunk in value.chunks(4) {
                let x = chunk[0] as i32;
                let y = chunk[1] as i32;
                let width = chunk[2] as i32;
                let height = chunk[3] as i32;
                monitors.push(Monitor { x, y, width, height });
            }
            return Ok(monitors);
        }

        Err("Failed to get monitors".into())
    }
}

pub enum Layout {
    Horizontal,
    Vertical,
}

struct Monitor {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}
