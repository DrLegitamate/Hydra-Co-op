use std::net::{UdpSocket, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use log::{info, error};
use std::env;
use std::sync::mpsc::{self, Sender};
use std::thread;

pub struct NetEmulator {
    sockets: Arc<RwLock<HashMap<u8, UdpSocket>>>,
    mappings: Arc<RwLock<HashMap<SocketAddr, SocketAddr>>>,
    stop_tx: Option<Sender<()>>,
}

impl NetEmulator {
    pub fn new() -> Self {
        NetEmulator {
            sockets: Arc::new(RwLock::new(HashMap::new())),
            mappings: Arc::new(RwLock::new(HashMap::new())),
            stop_tx: None,
        }
    }

    pub fn add_instance(&self, instance_id: u8) -> Result<(), std::io::Error> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        info!("Instance {} bound to port {}", instance_id, port);

        let mut sockets = self.sockets.write().unwrap();
        sockets.insert(instance_id, socket);

        Ok(())
    }

    pub fn add_mapping(&self, src: SocketAddr, dst: SocketAddr) {
        let mut mappings = self.mappings.write().unwrap();
        mappings.insert(src, dst);
        info!("Added mapping from {} to {}", src, dst);
    }

    pub fn start_relay(&mut self) {
        // Log the start of the relay
        info!("Starting network packet relay");

        let sockets = Arc::clone(&self.sockets);
        let mappings = Arc::clone(&self.mappings);
        let (stop_tx, stop_rx) = mpsc::channel();
        self.stop_tx = Some(stop_tx);

        thread::spawn(move || {
            let mut buf = [0; 1024];
            loop {
                for (instance_id, socket) in sockets.read().unwrap().iter() {
                    match socket.recv_from(&mut buf) {
                        Ok((size, src)) => {
                            info!("Received {} bytes from {}", size, src);
                            let dst = mappings.read().unwrap().get(&src).cloned();
                            if let Some(dst) = dst {
                                if let Err(e) = socket.send_to(&buf[..size], dst) {
                                    error!("Failed to send to {}: {}", dst, e);
                                } else {
                                    info!("Forwarded {} bytes to {}", size, dst);
                                }
                            } else {
                                error!("No mapping found for {}", src);
                            }
                        }
                        Err(e) => {
                            error!("Failed to receive from socket {}: {}", instance_id, e);
                        }
                    }
                }

                // Check for stop signal
                if stop_rx.try_recv().is_ok() {
                    info!("Stopping network packet relay");
                    break;
                }
            }
        });
    }

    pub fn stop_relay(&self) {
        if let Some(stop_tx) = &self.stop_tx {
            stop_tx.send(()).unwrap();
        }
    }
}
