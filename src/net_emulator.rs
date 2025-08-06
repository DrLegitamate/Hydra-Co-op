use std::net::{UdpSocket, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use log::{info, error, warn, debug};
use std::io::{self, Read}; // Import Read trait for potential error handling
use std::sync::mpsc::{self, Sender, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;
use std::error::Error; // Import Error trait

// We will use the 'polling' crate for handling multiple non-blocking sockets.
// Add this to your Cargo.toml:
// [dependencies]
// polling = "2.3" # Or the latest version

// Custom error type for network emulation operations
#[derive(Debug)]
pub enum NetEmulatorError {
    IoError(io::Error),
    GenericError(String),
    PollingError(polling::Error),
    ChannelError(mpsc::SendError<()>), // For errors sending on the stop channel
}

impl std::fmt::Display for NetEmulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NetEmulatorError::IoError(e) => write!(f, "Network emulator I/O error: {}", e),
            NetEmulatorError::GenericError(msg) => write!(f, "Network emulator error: {}", msg),
            NetEmulatorError::PollingError(e) => write!(f, "Network emulator polling error: {}", e),
            NetEmulatorError::ChannelError(e) => write!(f, "Network emulator channel error: {}", e),
        }
    }
}

impl Error for NetEmulatorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NetEmulatorError::IoError(e) => Some(e),
            NetEmulatorError::PollingError(e) => Some(e),
            NetEmulatorError::ChannelError(e) => Some(e),
            _ => None,
        }
    }
}

// Implement From conversions for easier error propagation
impl From<io::Error> for NetEmulatorError {
    fn from(err: io::Error) -> Self {
        NetEmulatorError::IoError(err)
    }
}

impl From<polling::Error> for NetEmulatorError {
    fn from(err: polling::Error) -> Self {
        NetEmulatorError::PollingError(err)
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
            let mut buf = [0; 65507]; // Maximum theoretical UDP packet size

            // Create a poller instance
            let poller = polling::Poller::new().map_err(NetEmulatorError::PollingError)?;
            let mut event_queue = polling::Events::new(); // Event queue for polling results

            // Register all instance sockets with the poller
            { // Use a block to drop the read lock on sockets quickly
                let sockets_read = sockets.read().unwrap();
                for (instance_id, socket) in sockets_read.iter() {
                    // Register the socket for readable events
                    poller.add(socket, polling::Event::readable(*instance_id as usize)).map_err(NetEmulatorError::PollingError)?;
                    debug!("Registered socket for instance {} with poller.", instance_id);
                }
            } // Drop the read lock

            info!("Network relay thread started.");

            loop {
                // Check for stop signal from the main thread
                match stop_rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        info!("Stop signal received. Stopping network packet relay thread.");
                        break; // Exit the loop to stop the thread
                    }
                    Err(TryRecvError::Empty) => {
                        // No stop signal, continue
                    }
                }

                // Wait for events on registered sockets with a timeout to check the stop channel periodically
                // A small timeout prevents busy-waiting but allows responsiveness to stop signals.
                match poller.wait(&mut event_queue, Some(Duration::from_millis(100))) {
                    Ok(num_events) => {
                        // Process events
                        for i in 0..num_events {
                            let event = event_queue.get(i).unwrap();
                            let instance_id = event.key as u8; // The key is the instance ID

                            debug!("Received polling event for instance {}", instance_id);

                            // Get the socket for this instance (acquire read lock)
                            let sockets_read = sockets.read().unwrap();
                            if let Some(socket) = sockets_read.get(&instance_id) {
                                // Attempt to receive packets from the non-blocking socket in a loop
                                // as multiple packets might be available.
                                loop {
                                    match socket.recv_from(&mut buf) {
                                        Ok((size, src)) => {
                                            debug!("Received {} bytes from {} on socket for instance {}", size, src, instance_id);

                                            // Find the destination based on the mapping (acquire read lock on mappings)
                                            let mappings_read = mappings.read().unwrap();
                                            let dst_option = mappings_read.get(&src).cloned();
                                            drop(mappings_read); // Drop the read lock on mappings

                                            if let Some(dst) = dst_option {
                                                debug!("Forwarding {} bytes from {} to {} (instance {})", size, src, dst, instance_id);
                                                // Send the packet to the destination
                                                if let Err(e) = socket.send_to(&buf[..size], dst) {
                                                    // Log send errors, but don't stop the relay for this socket
                                                    error!("Failed to send {} bytes to {} for instance {}: {}", size, dst, instance_id, e);
                                                } else {
                                                     debug!("Forwarded {} bytes successfully.", size);
                                                }
                                            } else {
                                                debug!("No mapping found for source address {} (instance {}). Packet dropped.", src, instance_id);
                                                // Packet is dropped if no mapping exists
                                            }
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            // No more packets available to read from this socket right now
                                            debug!("Socket for instance {} is non-blocking and reported WouldBlock.", instance_id);
                                            break; // Exit the inner loop for this socket
                                        }
                                        Err(e) => {
                                            // Handle other receive errors (e.g., socket closed, network issues)
                                            error!("Error receiving from socket for instance {}: {}", instance_id, e);
                                            // Depending on the error, you might want to deregister the socket
                                            // or stop the relay thread if it's a critical error.
                                            // For now, we just log and continue checking other sockets.
                                            break; // Exit the inner loop for this socket on error
                                        }
                                    }
                                } // End of inner recv_from loop

                                // Re-register the socket after handling events, as some polling mechanisms
                                // require this to continue receiving events.
                                // Ensure the socket is still valid before re-registering.
                                 if let Some(valid_socket) = sockets_read.get(&instance_id) {
                                      if let Err(e) = poller.modify(valid_socket, polling::Event::readable(*instance_id as usize)) {
                                           // Log error if re-registration fails (e.g., socket is no longer valid)
                                           error!("Failed to re-register socket for instance {} with poller: {}", instance_id, e);
                                           // Depending on the error, you might want to try removing it from the poller
                                           // or assume the instance/socket is gone.
                                      }
                                 } else {
                                      // The socket might have been removed from the map (e.g., instance stopped)
                                      debug!("Socket for instance {} not found in map during re-registration.", instance_id);
                                      // Consider cleaning up the poller registration for this instance ID here.
                                 }

                            } else {
                                // Should not happen if instance_id comes from poller events based on sockets map
                                error!("Internal error: Socket for instance ID {} not found in map after polling event.", instance_id);
                            }
                             drop(sockets_read); // Drop the read lock on sockets
                        } // End of processing polling events
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                         // Poller timed out, or no events occurred. Continue the loop to check stop signal.
                         debug!("Poller wait timed out or no events.");
                    }
                    Err(e) => {
                        // Handle polling errors
                        error!("Polling error in network relay thread: {}", e);
                        // Depending on the error, you might break the loop or try to recover
                        return Err(NetEmulatorError::PollingError(e)); // Return the error to the main thread
                    }
                } // End of poller.wait match
            } // End of main relay loop

             // Clean up poller resources (poller is dropped when the thread exits)
            info!("Network relay thread stopped gracefully.");
             Ok(()) // Return Ok on successful stop
        });

        self.relay_thread = Some(relay_thread);

        Ok(())
    }

    /// Get join handle for relay thread (for external thread management)
    pub fn join_relay(&mut self) -> Option<thread::JoinHandle<Result<(), NetEmulatorError>>> {
        self.relay_thread.take()
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