// kernel/src/sched.rs
// Written in Rust (no_std)
// Thread scheduling queues (READY, RUNNING, BLOCKED, ZOMBIE).
//
// no_std changes:
//   - std::collections::VecDeque → alloc::collections::VecDeque
//   - std::sync::{Arc, Mutex}   → alloc::sync::Arc + spin::Mutex
//   - println!()                 → kprint! (VGA output via FFI)

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::string::{String, ToString};
use spin::Mutex;
use crate::kprint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Zombie,
}

#[derive(Debug, Default, Clone)]
pub struct RegistersContext {
    pub rip:    u64,
    pub rsp:    u64,
    pub rbp:    u64,
    pub rflags: u64,
    pub rax:    u64,
    pub rdi:    u64,
    pub rsi:    u64,
}

pub struct Thread {
    pub id:         u32,
    pub session_id: u64,
    pub name:       String,
    pub state:      ThreadState,
    pub context:    RegistersContext,
}

pub struct Scheduler {
    pub threads:         VecDeque<Arc<Mutex<Thread>>>,
    pub next_thread_id:  u32,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            threads: VecDeque::new(),
            next_thread_id: 1,
        }
    }

    pub fn spawn(&mut self, session_id: u64, name: &str, entry_point: u64) -> u32 {
        let t_id = self.next_thread_id;
        self.next_thread_id += 1;

        let mut context = RegistersContext::default();
        context.rip = entry_point;
        context.rsp = 0x7FFFFFFF0000; // Default user stack top

        let thread = Arc::new(Mutex::new(Thread {
            id: t_id,
            session_id,
            name: name.to_string(),
            state: ThreadState::Ready,
            context,
        }));

        self.threads.push_back(thread);
        kprint!("[Scheduler] Spawned Thread {} for Session {}\n", t_id, session_id);
        t_id
    }

    pub fn schedule_next(&mut self) -> Option<Arc<Mutex<Thread>>> {
        if self.threads.is_empty() {
            return None;
        }

        for _ in 0..self.threads.len() {
            let thread_arc = self.threads.pop_front()?;
            let mut thread = thread_arc.lock();

            if thread.state == ThreadState::Ready {
                thread.state = ThreadState::Running;
                kprint!("[Scheduler] Context Switch -> Thread {}\n", thread.id);
                thread.state = ThreadState::Ready; // Preempted — back to ready
                drop(thread);
                self.threads.push_back(Arc::clone(&thread_arc));
                return Some(thread_arc);
            } else {
                drop(thread);
                self.threads.push_back(thread_arc);
            }
        }
        None
    }

    pub fn terminate_thread(&mut self, thread_id: u32) {
        for t_arc in &self.threads {
            let mut t = t_arc.lock();
            if t.id == thread_id {
                t.state = ThreadState::Zombie;
            }
        }
        self.threads.retain(|t_arc| {
            let t = t_arc.lock();
            t.state != ThreadState::Zombie
        });
    }
}
