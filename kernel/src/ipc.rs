// kernel/src/ipc.rs
// Written in Rust
// Inter-Process Communication (IPC) message ports and blocking queues.

use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub sender_session: u64,
    pub payload: String,
}

pub struct IpcChannel {
    pub port_id: u32,
    pub messages: VecDeque<IpcMessage>,
    pub blocked_threads: VecDeque<u32>, // list of TIDs blocked waiting for this port
}

pub struct IpcSubsystem {
    pub channels: HashMap<u32, IpcChannel>,
}

impl IpcSubsystem {
    pub fn new() -> Self {
        IpcSubsystem {
            channels: HashMap::new(),
        }
    }

    pub fn send(&mut self, port_id: u32, sender_session: u64, payload: &str) -> Option<u32> {
        let channel = self.channels.entry(port_id).or_insert_with(|| IpcChannel {
            port_id,
            messages: VecDeque::new(),
            blocked_threads: VecDeque::new(),
        });

        let msg = IpcMessage {
            sender_session,
            payload: payload.to_string(),
        };
        channel.messages.push_back(msg);
        println!("[IPC] Port {}: Session {} sent message: \"{}\"", port_id, sender_session, payload);

        // If there's a blocked thread waiting for this port, return its TID to wake it up
        channel.blocked_threads.pop_front()
    }

    pub fn recv(&mut self, port_id: u32, receiver_thread_id: u32) -> Result<Option<IpcMessage>, &'static str> {
        let channel = self.channels.entry(port_id).or_insert_with(|| IpcChannel {
            port_id,
            messages: VecDeque::new(),
            blocked_threads: VecDeque::new(),
        });

        if let Some(msg) = channel.messages.pop_front() {
            Ok(Some(msg))
        } else {
            // No message: Block the calling thread
            channel.blocked_threads.push_back(receiver_thread_id);
            println!("[IPC] Port {}: No message available. Thread {} blocked waiting.", port_id, receiver_thread_id);
            Ok(None)
        }
    }
}
