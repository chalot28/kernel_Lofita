// kernel/src/ipc.rs
// Written in Rust (no_std)
// Inter-Process Communication (IPC) message ports and blocking queues.
//
// no_std changes:
//   - std::collections::{HashMap, VecDeque} → hashbrown::HashMap + alloc::collections::VecDeque
//   - String                                → alloc::string::String
//   - println!                              → kprint!

use hashbrown::HashMap;
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use crate::kprint;

#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub sender_session: u64,
    pub payload:        String,
}

pub struct IpcChannel {
    pub port_id:         u32,
    pub messages:        VecDeque<IpcMessage>,
    pub blocked_threads: VecDeque<u32>, // TIDs waiting on this port
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
            messages:        VecDeque::new(),
            blocked_threads: VecDeque::new(),
        });

        if channel.messages.len() >= 1024 {
            kprint!("[IPC] Port {}: Queue full. Dropping message.\n", port_id);
            return None;
        }

        let msg = IpcMessage {
            sender_session,
            payload: payload.to_string(),
        };
        channel.messages.push_back(msg);
        kprint!("[IPC] Port {}: Session {} sent message.\n", port_id, sender_session);

        // Wake a blocked receiver if one exists
        channel.blocked_threads.pop_front()
    }

    pub fn recv(&mut self, port_id: u32, receiver_thread_id: u32) -> Result<Option<IpcMessage>, &'static str> {
        let channel = self.channels.entry(port_id).or_insert_with(|| IpcChannel {
            port_id,
            messages:        VecDeque::new(),
            blocked_threads: VecDeque::new(),
        });

        if let Some(msg) = channel.messages.pop_front() {
            Ok(Some(msg))
        } else {
            // Block the calling thread
            channel.blocked_threads.push_back(receiver_thread_id);
            kprint!("[IPC] Port {}: Thread {} blocked waiting.\n", port_id, receiver_thread_id);
            Ok(None)
        }
    }
}
