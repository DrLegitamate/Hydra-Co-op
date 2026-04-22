use std::net::{UdpSocket, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use log::{info, error, warn, debug};
use std::io;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use std::error::Error;

// Custom error type for network emulation operations
#[derive(Debug)]
pub enum NetEmulatorError {
    IoError(io::Error),
    GenericError(String),
    ChannelError(mpsc::SendError<()>),
}

impl std::fmt::Display for NetEmulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NetEmulatorError::IoError(e) => write!(f, "Network emulator I/O error: {}", e),
            NetEmulatorError::GenericError(msg) => write!(f, "Network emulator error: {}", msg),
            NetEmulatorError::ChannelError(e) => write!(f, "Network emulator channel error: {}", e),
        }
    }
}

impl Error for NetEmulatorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NetEmulatorError::IoError(e) => Some(e),
            NetEmulatorError::ChannelError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NetEmulatorError {
    fn from(err: io::Error) -> Self {
        NetEmulatorError::IoError(err)
    }
}

impl From<mpsc::SendError<()>> for NetEmulatorError {
    fn from(err: mpsc::SendError<()>) -> Self {
        NetEmulatorError::ChannelError(err)
    }
}


/// Represents a network emulator for relaying UDP packets between game instances.
pub struct NetEmulator {
    // Map instance ID to its UDP socket
    sockets: Arc<RwLock<HashMap<u8, UdpSocket>>>,
    // Map source SocketAddr to destination SocketAddr for relaying
    mappings: Arc<RwLock<HashMap<SocketAddr, SocketAddr>>>,
    // Channel sender to signal the relay thread to stop
    stop_tx: Option<Sender<()>>,
    // Join handle for the relay thread
    relay_thread: Option<thread::JoinHandle<Result<(), NetEmulatorError>>>,
}

impl NetEmulator {
    pub fn new() -> Self {
        NetEmulator {
            sockets: Arc::new(RwLock::new(HashMap::new())),
            mappings: Arc::new(RwLock::new(HashMap::new())),
            stop_tx: None,
            relay_thread: None,
        }
    }

    /// Adds a new game instance to the network emulator by binding a UDP socket.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - A unique identifier for the game instance (0, 1, 2, ...).
    ///
    /// # Returns
    ///
    /// * `Result<u16, NetEmulatorError>` - Returns the bound port number if successful,
    ///   otherwise returns a NetEmulatorError.
    pub fn add_instance(&self, instance_id: u8) -> Result<u16, NetEmulatorError> {
        // Bind to 127.0.0.1 with port 0, letting the OS choose a free port
        let socket = UdpSocket::bind("127.0.0.1:0").map_err(NetEmulatorError::IoError)?;
        let port = socket.local_addr().map_err(NetEmulatorError::IoError)?.port();

        // Set the socket to non-blocking mode for use with polling
        socket.set_nonblocking(true).map_err(NetEmulatorError::IoError)?;

        info!("Instance {} bound to port {}", instance_id, port);

        let mut sockets = self.sockets.write().unwrap();
        sockets.insert(instance_id, socket);

        Ok(port) // Return the bound port number
    }

    /// Adds a network mapping from a source address to a destination address.
    /// Packets received from `src` will be forwarded to `dst`.
    ///
    /// # Arguments
    ///
    /// * `src` - The source SocketAddr (IP and port) to listen for packets from.
    /// * `dst` - The destination SocketAddr (IP and port) to forward packets to.
    pub fn add_mapping(&self, src: SocketAddr, dst: SocketAddr) {
        let mut mappings = self.mappings.write().unwrap();
        mappings.insert(src, dst);
        info!("Added mapping from {} to {}", src, dst);
    }

    /// Starts a background thread to relay network packets between instance sockets
    /// based on the configured mappings. Uses non-blocking sockets and polling
    /// for efficient handling of multiple connections.
    pub fn start_relay(&mut self) -> Result<(), NetEmulatorError> {
        // Avoid starting multiple relay threads
        if self.relay_thread.is_some() {
            warn!("Network packet relay is already running.");
            return Ok(());
        }

        info!("Starting network packet relay thread.");

        let sockets = Arc::clone(&self.sockets);
        let mappings = Arc::clone(&self.mappings);
        let (stop_tx, stop_rx) = mpsc::channel();
        self.stop_tx = Some(stop_tx);

        let relay_thread = thread::spawn(move || {
            let mut buf = [0; 65507];

            let poller = polling::Poller::new()?;
            let mut event_queue = polling::Events::new();

            // Register all instance sockets with the poller. `add` requires a raw
            // source; callers must guarantee the source outlives the poller, which
            // holds here because the sockets live in the Arc<RwLock> we cloned.
            {
                let sockets_read = sockets.read().unwrap();
                for (instance_id, socket) in sockets_read.iter() {
                    unsafe {
                        poller.add(socket, polling::Event::readable(*instance_id as usize))?;
                    }
                    debug!("Registered socket for instance {} with poller.", instance_id);
                }
            }

            info!("Network relay thread started.");

            loop {
                match stop_rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        info!("Stop signal received. Stopping network packet relay thread.");
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }

                match poller.wait(&mut event_queue, Some(Duration::from_millis(100))) {
                    Ok(_) => {
                        for event in event_queue.iter() {
                            let instance_id = event.key as u8;
                            debug!("Received polling event for instance {}", instance_id);

                            let sockets_read = sockets.read().unwrap();
                            if let Some(socket) = sockets_read.get(&instance_id) {
                                loop {
                                    match socket.recv_from(&mut buf) {
                                        Ok((size, src)) => {
                                            debug!("Received {} bytes from {} on socket for instance {}", size, src, instance_id);

                                            let mappings_read = mappings.read().unwrap();
                                            let dst_option = mappings_read.get(&src).cloned();
                                            drop(mappings_read);

                                            if let Some(dst) = dst_option {
                                                debug!("Forwarding {} bytes from {} to {} (instance {})", size, src, dst, instance_id);
                                                if let Err(e) = socket.send_to(&buf[..size], dst) {
                                                    error!("Failed to send {} bytes to {} for instance {}: {}", size, dst, instance_id, e);
                                                } else {
                                                    debug!("Forwarded {} bytes successfully.", size);
                                                }
                                            } else {
                                                debug!("No mapping found for source address {} (instance {}). Packet dropped.", src, instance_id);
                                            }
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            debug!("Socket for instance {} is non-blocking and reported WouldBlock.", instance_id);
                                            break;
                                        }
                                        Err(e) => {
                                            error!("Error receiving from socket for instance {}: {}", instance_id, e);
                                            break;
                                        }
                                    }
                                }

                                // Re-arm oneshot interest so future packets keep waking the poller.
                                if let Err(e) = poller.modify(socket, polling::Event::readable(instance_id as usize)) {
                                    error!("Failed to re-register socket for instance {} with poller: {}", instance_id, e);
                                }
                            } else {
                                error!("Internal error: Socket for instance ID {} not found in map after polling event.", instance_id);
                            }
                            drop(sockets_read);
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        debug!("Poller wait timed out or no events.");
                    }
                    Err(e) => {
                        error!("Polling error in network relay thread: {}", e);
                        return Err(NetEmulatorError::IoError(e));
                    }
                }
            }

            info!("Network relay thread stopped gracefully.");
            Ok(())
        });

        self.relay_thread = Some(relay_thread);

        Ok(())
    }

    /// Sends a stop signal to the relay thread and waits for it to finish.
    pub fn stop_relay(&mut self) -> Result<(), NetEmulatorError> {
        info!("Stopping network packet relay thread.");
        // Send stop signal
        if let Some(stop_tx) = self.stop_tx.take() { // Take the sender to prevent sending again
             stop_tx.send(()).map_err(NetEmulatorError::ChannelError)?;
             debug!("Stop signal sent.");
        } else {
             warn!("Network packet relay thread was not running or already stopped.");
             return Ok(()); // Nothing to stop
        }

        // Wait for the relay thread to finish
        if let Some(relay_thread) = self.relay_thread.take() { // Take the join handle
            match relay_thread.join() {
                Ok(thread_result) => {
                    // Check the result the thread returned
                    match thread_result {
                        Ok(_) => info!("Network relay thread joined successfully."),
                        Err(e) => {
                             error!("Network relay thread finished with an error: {}", e);
                             return Err(e); // Return the error from the thread
                        }
                    }
                }
                Err(e) => {
                    error!("Network relay thread panicked: {:?}", e);
                    return Err(NetEmulatorError::GenericError(format!("Network relay thread panicked: {:?}", e))); // Return a generic error for panic
                }
            }
        }
        info!("Network packet relay stopped.");
        Ok(())
    }
}

// Ensure stop_relay is called when NetEmulator is dropped
impl Drop for NetEmulator {
    fn drop(&mut self) {
        // Attempt to stop the relay thread when the NetEmulator goes out of scope
        if self.relay_thread.is_some() {
             warn!("NetEmulator is being dropped, but relay thread is still running. Attempting to stop relay.");
             if let Err(e) = self.stop_relay() {
                  error!("Error during NetEmulator drop while stopping relay: {}", e);
             }
        }
    }
}


// Test code moved into a test module
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;

    // Note: Testing network relay logic requires binding to ports and sending/receiving
    // actual network packets, which can be challenging in a pure unit test environment
    // and might require elevated privileges or specific network configurations.
    // These would typically be integration tests.

    #[test]
    fn test_add_instance() {
        let emulator = NetEmulator::new();
        let result1 = emulator.add_instance(0);
        let result2 = emulator.add_instance(1);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let port1 = result1.unwrap();
        let port2 = result2.unwrap();
        assert_ne!(port1, port2, "Instances should bind to different ports");

        // Check if sockets were added to the map
        let sockets = emulator.sockets.read().unwrap();
        assert_eq!(sockets.len(), 2);
        assert!(sockets.contains_key(&0));
        assert!(sockets.contains_key(&1));

        // Ensure sockets are non-blocking (check requires accessing internal state, less ideal)
        // A robust test might involve trying a non-blocking receive.
    }

    #[test]
    fn test_add_mapping() {
        let emulator = NetEmulator::new();
        let src1: SocketAddr = "127.0.0.1:10001".parse().unwrap();
        let dst1: SocketAddr = "127.0.0.1:10002".parse().unwrap();
        let src2: SocketAddr = "127.0.0.1:10003".parse().unwrap();
        let dst2: SocketAddr = "127.0.0.1:10004".parse().unwrap();

        emulator.add_mapping(src1, dst1);
        emulator.add_mapping(src2, dst2);

        let mappings = emulator.mappings.read().unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings.get(&src1), Some(&dst1));
        assert_eq!(mappings.get(&src2), Some(&dst2));
    }

    #[test]
    #[ignore] // Ignoring as it requires starting a thread and potential network setup
    fn test_start_and_stop_relay() {
        let mut emulator = NetEmulator::new();
        let start_result = emulator.start_relay();
        assert!(start_result.is_ok());

        // Allow some time for the thread to start
        thread::sleep(Duration::from_millis(50));

        let stop_result = emulator.stop_relay();
        assert!(stop_result.is_ok());

        // Allow some time for the thread to finish
        thread::sleep(Duration::from_millis(50));

        // Check that the relay thread handle is None after stopping
        assert!(emulator.relay_thread.is_none());

        // Attempting to stop again should be ok but log a warning
        let stop_again_result = emulator.stop_relay();
        assert!(stop_again_result.is_ok());
    }

    // Add more integration tests for packet relaying if feasible.
}