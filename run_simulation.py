#!/usr/bin/env python3
import time
import sys

# ANSI Colors for beautiful logs
COLOR_KERNEL = "\033[95m"   # Purple
COLOR_ZIG = "\033[96m"      # Cyan (Zig allocator)
COLOR_RUST = "\033[94m"     # Blue (Rust VASM/Token Manager)
COLOR_WINE = "\033[93m"     # Yellow (lorifa-wine-host)
COLOR_APP = "\033[92m"      # Green (User App)
COLOR_ALERT = "\033[91m"    # Red (Warnings/Expirations)
COLOR_SHELL = "\033[97;1m"  # White Bold
COLOR_RESET = "\033[0m"

PAGE_SIZE = 4096  # 4 KB
MEMORY_SIZE = 64 * 1024 * 1024  # 64 MB
TOTAL_PAGES = MEMORY_SIZE // PAGE_SIZE  # 16384 pages
MAX_ORDER = 11

def log_kernel(msg):
    print(f"{COLOR_KERNEL}[Lorifa Kernel]{COLOR_RESET} {msg}")

def log_zig(msg):
    print(f"{COLOR_ZIG}[Zig PPA]{COLOR_RESET} {msg}")

def log_rust(msg):
    print(f"{COLOR_RUST}[Rust Core]{COLOR_RESET} {msg}")

def log_wine(msg):
    print(f"{COLOR_WINE}[lorifa-wine-host]{COLOR_RESET} {msg}")

def log_app(name, msg):
    print(f"{COLOR_APP}[App: {name}]{COLOR_RESET} {msg}")

def log_alert(msg):
    print(f"{COLOR_ALERT}[KERNEL ALARM]{COLOR_RESET} {msg}")


class BuddyAllocator:
    """Simulates the Physical Page Allocator written in Zig (mm/ppa.zig)."""
    def __init__(self):
        self.pages = [{"order": 0, "is_free": True} for _ in range(TOTAL_PAGES)]
        max_block_pages = 1 << (MAX_ORDER - 1)
        for i in range(0, TOTAL_PAGES, max_block_pages):
            self.pages[i]["order"] = MAX_ORDER - 1

    def alloc(self, num_pages):
        if num_pages == 0:
            return None
        
        req_order = 0
        while (1 << req_order) < num_pages:
            req_order += 1
            if req_order >= MAX_ORDER:
                return None

        current_order = req_order
        while current_order < MAX_ORDER:
            block_size = 1 << current_order
            for i in range(0, TOTAL_PAGES, block_size):
                if self.pages[i]["is_free"] and self.pages[i]["order"] == current_order:
                    while current_order > req_order:
                        current_order -= 1
                        half_size = 1 << current_order
                        buddy_idx = i + half_size
                        self.pages[i]["order"] = current_order
                        self.pages[buddy_idx]["order"] = current_order
                        self.pages[buddy_idx]["is_free"] = True
                    
                    self.pages[i]["is_free"] = False
                    phys_addr = i * PAGE_SIZE
                    log_zig(f"Allocated {num_pages} page(s) (Order {req_order}) at physical address 0x{phys_addr:08X}")
                    return phys_addr
            current_order += 1
        return None

    def free(self, phys_addr, num_pages):
        if phys_addr is None or phys_addr % PAGE_SIZE != 0:
            return
        
        start_page = phys_addr // PAGE_SIZE
        if start_page >= TOTAL_PAGES:
            return

        req_order = 0
        while (1 << req_order) < num_pages:
            req_order += 1

        current_order = req_order
        page_idx = start_page
        self.pages[page_idx]["is_free"] = True
        log_zig(f"Freeing {num_pages} page(s) at physical address 0x{phys_addr:08X}")

        while current_order < MAX_ORDER - 1:
            block_size = 1 << current_order
            buddy_idx = page_idx ^ block_size
            if buddy_idx >= TOTAL_PAGES:
                break
            
            if self.pages[buddy_idx]["is_free"] and self.pages[buddy_idx]["order"] == current_order:
                self.pages[buddy_idx]["is_free"] = False
                if buddy_idx < page_idx:
                    page_idx = buddy_idx
                current_order += 1
                self.pages[page_idx]["order"] = current_order
                self.pages[page_idx]["is_free"] = True
                log_zig(f"Merged buddy block at 0x{buddy_idx*PAGE_SIZE:08X} into Order {current_order} block at 0x{page_idx*PAGE_SIZE:08X}")
            else:
                break

    def get_status(self):
        free_pages = sum(1 for p in self.pages if p["is_free"])
        allocated_pages = TOTAL_PAGES - free_pages
        blocks_by_order = [0] * MAX_ORDER
        for i in range(TOTAL_PAGES):
            if self.pages[i]["is_free"]:
                order = self.pages[i]["order"]
                block_size = 1 << order
                if i % block_size == 0:
                    blocks_by_order[order] += 1
                    
        return {
            "free_bytes": free_pages * PAGE_SIZE,
            "allocated_bytes": allocated_pages * PAGE_SIZE,
            "free_pages": free_pages,
            "allocated_pages": allocated_pages,
            "blocks_by_order": blocks_by_order
        }


class PagingContext:
    """Simulates x86_64 nested page tables (arch/x86_64/mm/paging.zig)."""
    def __init__(self):
        self.mappings = {}

    def map(self, virtual_addr, physical_addr, size):
        pages = (size + PAGE_SIZE - 1) // PAGE_SIZE
        for i in range(pages):
            v = virtual_addr + i * PAGE_SIZE
            p = physical_addr + i * PAGE_SIZE
            self.mappings[v] = p
            
            pml4 = (v >> 39) & 0x1FF
            pdpt = (v >> 30) & 0x1FF
            pd = (v >> 21) & 0x1FF
            pt = (v >> 12) & 0x1FF
            log_zig(f"  Paging: PML4[{pml4}] -> PDPT[{pdpt}] -> PD[{pd}] -> PT[{pt}] mapped to physical 0x{p:08X}")

    def unmap(self, virtual_addr, size):
        pages = (size + PAGE_SIZE - 1) // PAGE_SIZE
        for i in range(pages):
            v = virtual_addr + i * PAGE_SIZE
            if v in self.mappings:
                del self.mappings[v]
            log_zig(f"  Assembly: invlpg (0x{v:08X}) executed to invalidate TLB entry.")
        log_zig(f"  Paging: Invalidated TLB for virtual address range 0x{virtual_addr:08X}")


class Capability:
    MEM_ALLOC = 1 << 0
    MEM_FREE = 1 << 1
    FS_READ = 1 << 2
    FS_WRITE = 1 << 3
    NET_CONNECT = 1 << 4
    NET_BIND = 1 << 5
    SYS_ADMIN = 1 << 6
    DRV_MMIO = 1 << 7
    WINE_BRIDGE = 1 << 8

    @staticmethod
    def parse_list(cap_str):
        if not cap_str or cap_str.upper() == "NONE":
            return 0
        if cap_str.upper() == "ALL":
            return 0x1FF
            
        mask = 0
        parts = [p.strip().upper() for p in cap_str.split(",")]
        for p in parts:
            if p == "MEM_ALLOC": mask |= Capability.MEM_ALLOC
            elif p == "MEM_FREE": mask |= Capability.MEM_FREE
            elif p == "FS_READ": mask |= Capability.FS_READ
            elif p == "FS_WRITE": mask |= Capability.FS_WRITE
            elif p == "NET_CONNECT": mask |= Capability.NET_CONNECT
            elif p == "NET_BIND": mask |= Capability.NET_BIND
            elif p == "SYS_ADMIN": mask |= Capability.SYS_ADMIN
            elif p == "DRV_MMIO": mask |= Capability.DRV_MMIO
            elif p == "WINE_BRIDGE": mask |= Capability.WINE_BRIDGE
            else:
                raise ValueError(f"Unknown capability: {p}")
        return mask

    @staticmethod
    def to_string(cap_mask):
        names = []
        if cap_mask & Capability.MEM_ALLOC: names.append("MEM_ALLOC")
        if cap_mask & Capability.MEM_FREE: names.append("MEM_FREE")
        if cap_mask & Capability.FS_READ: names.append("FS_READ")
        if cap_mask & Capability.FS_WRITE: names.append("FS_WRITE")
        if cap_mask & Capability.NET_CONNECT: names.append("NET_CONNECT")
        if cap_mask & Capability.NET_BIND: names.append("NET_BIND")
        if cap_mask & Capability.SYS_ADMIN: names.append("SYS_ADMIN")
        if cap_mask & Capability.DRV_MMIO: names.append("DRV_MMIO")
        if cap_mask & Capability.WINE_BRIDGE: names.append("WINE_BRIDGE")
        return ", ".join(names) if names else "NONE"


class PrivilegeLevel:
    ROOT = 0
    ADMIN = 1
    USER = 2
    PROCESS = 3

    @staticmethod
    def parse(level_str):
        l = level_str.upper()
        if l == "ROOT": return PrivilegeLevel.ROOT
        if l == "ADMIN": return PrivilegeLevel.ADMIN
        if l == "USER": return PrivilegeLevel.USER
        if l == "PROCESS": return PrivilegeLevel.PROCESS
        raise ValueError(f"Unknown privilege: {level_str}")

    @staticmethod
    def to_string(level):
        return ["Root", "Admin", "User", "Process"][level]


class Vma:
    def __init__(self, start_addr, size, phys_ptr, is_writeable=True, is_executable=False):
        self.start_addr = start_addr
        self.size = size
        self.phys_ptr = phys_ptr
        self.is_writeable = is_writeable
        self.is_executable = is_executable


class Token:
    def __init__(self, token_id, name, privilege, parent_id, memory_limit, is_permanent, lifetime_seconds, capabilities):
        self.id = token_id
        self.name = name
        self.privilege = privilege
        self.parent_id = parent_id
        self.memory_limit = memory_limit
        self.memory_used = 0
        self.is_permanent = is_permanent
        self.expiry = time.time() + lifetime_seconds
        self.lifetime = lifetime_seconds
        self.capabilities = capabilities
        self.vmas = []
        self.run_count = 0
        self.is_deprecated = False


class Session:
    def __init__(self, session_id, token, lifetime_seconds):
        self.id = session_id
        self.token = token
        self.expiry = time.time() + lifetime_seconds
        self.is_active = True


class Thread:
    """Thread Control Block (TCB) simulated in Rust core (sched.rs)."""
    def __init__(self, thread_id, session_id, name, rip):
        self.id = thread_id
        self.session_id = session_id
        self.name = name
        self.rip = rip
        self.rsp = 0x7FFFFFFF0000
        self.state = "READY" # READY, RUNNING, BLOCKED, ZOMBIE


class Scheduler:
    """Round-robin Process Scheduler simulated in Rust core (sched.rs)."""
    def __init__(self):
        self.threads = []
        self.next_thread_id = 1

    def spawn(self, session_id, name, rip=0x10000000):
        t_id = self.next_thread_id
        self.next_thread_id += 1
        t = Thread(t_id, session_id, name, rip)
        self.threads.append(t)
        log_rust(f"Scheduler: Spawned Thread {t_id} ('{name}') for Session {session_id}")
        return t_id

    def schedule_next(self):
        if not self.threads:
            return None
        
        # Cycle through threads to find one READY
        for _ in range(len(self.threads)):
            t = self.threads.pop(0)
            if t.state == "READY":
                t.state = "RUNNING"
                log_rust(f"Scheduler: Context Switch -> Running Thread {t.id} ('{t.name}') [RIP: 0x{t.rip:08X}]")
                t.state = "READY"
                self.threads.append(t)
                return t
            self.threads.append(t)
        return None

    def terminate_session_threads(self, session_id):
        log_rust(f"Scheduler: Terminating all threads for Session {session_id}")
        self.threads = [t for t in self.threads if t.session_id != session_id]


# VFS Simulated Classes (vfs.rs)
class FileDescriptor:
    def __init__(self, fd, path, is_writeable):
        self.fd = fd
        self.path = path
        self.is_writeable = is_writeable


class RamFile:
    def __init__(self, path, content):
        self.path = path
        self.content = content


class VfsSubsystem:
    def __init__(self):
        self.fd_tables = {} # session_id -> { fd -> FileDescriptor }
        self.ramfs = {}     # path -> RamFile
        self.next_fd = 3

    def open(self, session_id, path, is_write):
        if is_write and not path.startswith("/dev/") and path not in self.ramfs:
            self.ramfs[path] = RamFile(path, b"")
            log_rust(f"VFS: Created new RAMFS file '{path}'")

        if not path.startswith("/dev/") and path not in self.ramfs:
            raise FileNotFoundError("File not found in RAMFS")

        table = self.fd_tables.setdefault(session_id, {})
        fd = self.next_fd
        self.next_fd += 1
        table[fd] = FileDescriptor(fd, path, is_write)
        log_rust(f"VFS: Session {session_id} opened file '{path}' -> assigned FD {fd}")
        return fd

    def close(self, session_id, fd):
        table = self.fd_tables.get(session_id, {})
        if fd in table:
            del table[fd]
            log_rust(f"VFS: Session {session_id} closed FD {fd}")
            return True
        return False

    def list_fds(self, session_id):
        return self.fd_tables.get(session_id, {})


# IPC Simulated Classes (ipc.rs)
class IpcMessage:
    def __init__(self, sender_session, payload):
        self.sender_session = sender_session
        self.payload = payload


class IpcChannel:
    def __init__(self, port_id):
        self.port_id = port_id
        self.messages = []
        self.blocked_threads = []


class IpcSubsystem:
    def __init__(self, scheduler):
        self.channels = {}
        self.scheduler = scheduler

    def send(self, port_id, sender_session, payload):
        channel = self.channels.setdefault(port_id, IpcChannel(port_id))
        msg = IpcMessage(sender_session, payload)
        channel.messages.append(msg)
        log_rust(f"IPC: Port {port_id}: Message sent -> \"{payload}\"")

        # If a thread is blocked waiting for this port, unblock it
        if channel.blocked_threads:
            tid = channel.blocked_threads.pop(0)
            for t in self.scheduler.threads:
                if t.id == tid:
                    t.state = "READY"
                    log_rust(f"IPC: Woke up Thread {tid} from Blocked queue.")
                    return tid
        return None

    def recv(self, port_id, thread_id):
        channel = self.channels.setdefault(port_id, IpcChannel(port_id))
        if channel.messages:
            msg = channel.messages.pop(0)
            log_rust(f"IPC: Thread {thread_id} read message payload: \"{msg.payload}\"")
            return msg
        else:
            # Block the thread
            channel.blocked_threads.append(thread_id)
            for t in self.scheduler.threads:
                if t.id == thread_id:
                    t.state = "BLOCKED"
            log_rust(f"IPC: Port {port_id} empty. Thread {thread_id} transitioned to BLOCKED state.")
            return None


class CharDriver:
    def open(self): return True
    def read(self, size): return b""
    def write(self, data): return len(data)

class NullDriver(CharDriver):
    def read(self, size): return b""
    def write(self, data): return len(data)

class UrandomDriver(CharDriver):
    def read(self, size):
        return bytes([(i * 33 + 7) % 256 for i in range(size)])
    def write(self, data): return len(data)

class Fb0Driver(CharDriver):
    def __init__(self):
        self.buffer = bytearray(4096)
    def read(self, size):
        limit = min(size, len(self.buffer))
        return bytes(self.buffer[0:limit])
    def write(self, data):
        limit = min(len(data), len(self.buffer))
        self.buffer[0:limit] = data[0:limit]
        print(f"[Driver fb0] Framebuffer updated with {limit} bytes.")
        return limit

class DriverManager:
    def __init__(self):
        self.drivers = {
            "/dev/null": NullDriver(),
            "/dev/urandom": UrandomDriver(),
            "/dev/fb0": Fb0Driver()
        }
    def read_device(self, path, size):
        if path in self.drivers:
            return self.drivers[path].read(size)
        return None
    def write_device(self, path, data):
        if path in self.drivers:
            return self.drivers[path].write(data)
        return None


class LorifaKernelSimulation:
    def __init__(self):
        self.ppa = BuddyAllocator()
        self.paging = PagingContext()
        self.scheduler = Scheduler()
        self.vfs = VfsSubsystem()
        self.ipc = IpcSubsystem(self.scheduler)
        self.driver_manager = DriverManager()
        self.tokens = {}
        self.sessions = {}
        self.next_token_id = 1
        self.next_session_id = 1
        self.boot_queue = []
        self.simulated_time_offset = 0.0
        
        # Initialize Root Token
        self.root_token_id = self.create_token(
            "RootSystem",
            PrivilegeLevel.ROOT,
            None,
            memory_limit=2**64,
            is_permanent=True,
            lifetime_seconds=999999,
            capabilities=0x1FF
        )

        # Decompress initramfs using RLE engine simulation
        initramfs_compressed = [
            1, ord('F'), 1, ord('I'), 1, ord('L'), 1, ord('E'), 1, ord(':'), 1, ord('/'), 1, ord('e'), 1, ord('t'), 1, ord('c'), 1, ord('/'), 1, ord('v'), 1, ord('e'), 1, ord('r'), 1, ord('s'), 1, ord('i'), 1, ord('o'), 1, ord('n'), 1, ord('\n'),
            1, ord('L'), 1, ord('o'), 1, ord('r'), 1, ord('i'), 1, ord('f'), 1, ord('a'), 1, ord(' '), 1, ord('M'), 1, ord('o'), 1, ord('n'), 1, ord('o'), 1, ord('l'), 1, ord('i'), 1, ord('t'), 1, ord('h'), 1, ord('i'), 1, ord('c'), 1, ord(' '), 1, ord('K'), 1, ord('e'), 1, ord('r'), 1, ord('n'), 1, ord('e'), 1, ord('l'), 1, ord(' '), 1, ord('v'), 1, ord('1'), 1, ord('.'), 1, ord('0'), 1, ord('.'), 1, ord('0'), 1, ord('\n'),
            1, ord('F'), 1, ord('I'), 1, ord('L'), 1, ord('E'), 1, ord(':'), 1, ord('/'), 1, ord('e'), 1, ord('t'), 1, ord('c'), 1, ord('/'), 1, ord('m'), 1, ord('o'), 1, ord('t'), 1, ord('d'), 1, ord('\n'),
            1, ord('W'), 1, ord('e'), 1, ord('l'), 1, ord('c'), 1, ord('o'), 1, ord('m'), 1, ord('e'), 1, ord(' '), 1, ord('t'), 1, ord('o'), 1, ord(' '), 1, ord('L'), 1, ord('o'), 1, ord('r'), 1, ord('i'), 1, ord('f'), 1, ord('a'), 1, ord(' '), 1, ord('O'), 1, ord('S'), 1, ord('!'), 1, ord('\n'),
            1, ord('F'), 1, ord('I'), 1, ord('L'), 1, ord('E'), 1, ord(':'), 1, ord('/'), 1, ord('e'), 1, ord('t'), 1, ord('c'), 1, ord('/'), 1, ord('h'), 1, ord('o'), 1, ord('s'), 1, ord('t'), 1, ord('s'), 1, ord('\n'),
            1, ord('1'), 1, ord('2'), 1, ord('7'), 1, ord('.'), 1, ord('0'), 1, ord('.'), 1, ord('0'), 1, ord('.'), 1, ord('1'), 1, ord(' '), 1, ord('l'), 1, ord('o'), 1, ord('c'), 1, ord('a'), 1, ord('l'), 1, ord('h'), 1, ord('o'), 1, ord('s'), 1, ord('t'), 1, ord('\n'),
        ]
        
        # Run RLE Decompress
        decompressed_data = bytearray()
        i = 0
        while i < len(initramfs_compressed):
            count = initramfs_compressed[i]
            byte = initramfs_compressed[i+1]
            decompressed_data.extend([byte] * count)
            i += 2
        
        # Parse decompressed payload
        text = decompressed_data.decode("utf-8")
        current_path = ""
        current_content = []
        for line in text.split("\n"):
            if line.startswith("FILE:"):
                if current_path:
                    file_body = "\n".join(current_content).encode("utf-8")
                    self.vfs.ramfs[current_path] = RamFile(current_path, file_body)
                current_path = line[len("FILE:"):].strip()
                current_content = []
            else:
                if line or current_path:
                    current_content.append(line)
        if current_path:
            file_body = "\n".join(current_content).encode("utf-8")
            self.vfs.ramfs[current_path] = RamFile(current_path, file_body)
            
        log_rust(f"Initramfs: Decompressed and loaded {len(self.vfs.ramfs)} files into RAMFS.")

    def current_time(self):
        return time.time() + self.simulated_time_offset

    def create_token(self, name, privilege, parent_id, memory_limit, is_permanent, lifetime_seconds, capabilities):
        if parent_id is not None:
            parent = self.tokens.get(parent_id)
            if not parent:
                raise ValueError("Parent token not found")
            
            if parent.privilege == PrivilegeLevel.PROCESS:
                raise PermissionError("Process token cannot spawn child tokens")
            elif parent.privilege == PrivilegeLevel.USER:
                if privilege != PrivilegeLevel.PROCESS:
                    raise PermissionError("User token can only spawn Process tokens")
                dangerous = Capability.SYS_ADMIN | Capability.DRV_MMIO | Capability.WINE_BRIDGE
                if capabilities & dangerous:
                    raise PermissionError("User cannot delegate administrative/dangerous capabilities")
            elif parent.privilege == PrivilegeLevel.ADMIN:
                if privilege == PrivilegeLevel.ROOT:
                    raise PermissionError("Admin cannot spawn Root tokens")
        else:
            if privilege != PrivilegeLevel.ROOT and len(self.tokens) > 0:
                raise PermissionError("Only Root token can be spawned without parent")

        t_id = self.next_token_id
        self.next_token_id += 1
        
        token = Token(t_id, name, privilege, parent_id, memory_limit, is_permanent, lifetime_seconds, capabilities)
        self.tokens[t_id] = token
        log_rust(f"Created Token {t_id} ('{name}', Privilege: {PrivilegeLevel.to_string(privilege)}, Caps: [{Capability.to_string(capabilities)}], Expiry: {lifetime_seconds}s)")
        return t_id

    def allocate_memory(self, token_id, size, is_writeable=True, is_executable=False):
        token = self.tokens.get(token_id)
        if not token:
            raise ValueError("Token not found")

        if not (token.capabilities & Capability.MEM_ALLOC):
            raise PermissionError("Token lacks MEM_ALLOC capability")

        if token.memory_used + size > token.memory_limit:
            raise MemoryError(f"Memory limit exceeded for Token {token_id}")

        # Enforce W^X (Write XOR Execute) security policy
        if is_writeable and is_executable:
            log_alert("Security Guard: W^X violation! Memory cannot be both Writeable and Executable.")
            raise PermissionError("W^X violation")
        log_rust("Security Guard: Enforcing W^X (Write XOR Execute) policy: Region is Writeable, Non-Executable. PASS.")

        pages_needed = (size + PAGE_SIZE - 1) // PAGE_SIZE
        actual_size = pages_needed * PAGE_SIZE

        # Allocate physical page
        phys_ptr = self.ppa.alloc(pages_needed)
        if phys_ptr is None:
            raise MemoryError("Physical memory exhausted")

        # Apply ASLR (Address Space Layout Randomization)
        def get_random_aslr_offset(seed):
            a = 1103515245
            c = 12345
            m = 1 << 31
            rand = (a * seed + c) % m
            return ((rand % 1024) * PAGE_SIZE)

        aslr_offset = get_random_aslr_offset(token.id ^ token.memory_used)
        sim_virtual_addr = 0x20000000 + (token.id * 0x1000000) + token.memory_used + aslr_offset
        self.paging.map(sim_virtual_addr, phys_ptr, actual_size)

        vma = Vma(sim_virtual_addr, actual_size, phys_ptr)
        token.vmas.append(vma)
        token.memory_used += actual_size

        log_rust(f"Token {token_id} allocated virtual 0x{sim_virtual_addr:08X} -> physical 0x{phys_ptr:08X} ({pages_needed} pages)")
        return sim_virtual_addr

    def run_process(self, token_id, session_lifetime):
        token = self.tokens.get(token_id)
        if not token:
            raise ValueError("Token not found")

        if not token.is_permanent and self.current_time() > token.expiry:
            raise PermissionError("Cannot run process: Token has expired")

        if token.run_count >= 2:
            raise PermissionError(f"Token {token_id} usage exceeded limit (2 runs max). Relaunch rejected.")

        token.run_count += 1
        log_rust(f"Launching process '{token.name}' using Token {token.id} (Run {token.run_count}/2)")

        if token.run_count == 1:
            log_rust(f"Generating replacement token request metadata for next launch of '{token.name}'.")
        elif token.run_count == 2:
            token.is_deprecated = True
            log_rust(f"Token {token_id} reached 2nd run. It will be deprecated upon exit.")

        s_id = self.next_session_id
        self.next_session_id += 1

        session = Session(s_id, token, session_lifetime)
        self.sessions[s_id] = session
        log_rust(f"Created Session {s_id} for Token {token_id} (Expires in {session_lifetime}s)")
        
        self.scheduler.spawn(s_id, token.name)
        return s_id

    def check_capability(self, session_id, cap_bit):
        session = self.sessions.get(session_id)
        if not session or not session.is_active:
            return False
        
        if self.current_time() > session.expiry:
            return False

        token = session.token
        if not token.is_permanent and self.current_time() > token.expiry:
            return False

        return bool(token.capabilities & cap_bit)

    def tick(self):
        now = self.current_time()
        expired_tokens = []

        # Check tokens
        for t_id, token in list(self.tokens.items()):
            if token.is_permanent:
                continue

            time_left = token.expiry - now
            if time_left <= 0:
                expired_tokens.append(t_id)
            elif time_left <= 5.0:
                log_alert(f"Token {t_id} ('{token.name}') will expire in {time_left:.2f}s! Sending SIGTOKENEXP.")

        # Reclaim expired tokens
        for t_id in expired_tokens:
            token = self.tokens.pop(t_id)
            log_kernel(f"Token {t_id} ('{token.name}') expired! Commencing memory reclamation.")

            for s_id, session in list(self.sessions.items()):
                if session.token.id == t_id:
                    session.is_active = False
                    self.scheduler.terminate_session_threads(s_id)
                    log_rust(f"Terminated Session {s_id} (Token {t_id} expired)")

            for vma in token.vmas:
                pages = vma.size // PAGE_SIZE
                log_rust(f"Reclaiming TMD virtual memory region 0x{vma.start_addr:08X}...")
                self.paging.unmap(vma.start_addr, vma.size)
                self.ppa.free(vma.phys_ptr, pages)
            token.vmas.clear()

        # Check sessions
        for s_id, session in list(self.sessions.items()):
            if session.is_active and now > session.expiry:
                session.is_active = False
                self.scheduler.terminate_session_threads(s_id)
                log_rust(f"Session {s_id} timed out. Revoked.")

        # Advance CPU Scheduling Round-Robin
        self.scheduler.schedule_next()


# Simulated Linux Syscall translation for Wine
class WineSyscallTranslator:
    def __init__(self, kernel, session_id, process_id):
        self.kernel = kernel
        self.session_id = session_id
        self.process_id = process_id
        self.heap_brk = 0x40000000
        self.heap_limit = 0x40000000

    def sys_open(self, path, is_write):
        log_wine(f"Syscall: sys_open(path='{path}', is_write={is_write})")
        cap = Capability.FS_WRITE if is_write else Capability.FS_READ
        if not self.kernel.check_capability(self.session_id, cap):
            log_wine("Syscall OPEN REJECTED: Lacks appropriate FS capability")
            return -13 # -EACCES
        try:
            fd = self.kernel.vfs.open(self.session_id, path, is_write)
            log_wine(f"Syscall OPEN: Opened '{path}' -> assigned Lorifa FD {fd}")
            return fd
        except Exception as e:
            return -2 # -ENOENT

    def sys_write(self, fd, count):
        log_wine(f"Syscall: sys_write(fd={fd}, count={count})")
        if not self.kernel.check_capability(self.session_id, Capability.FS_WRITE):
            log_wine(f"Syscall WRITE({fd}) REJECTED: Lacks FS_WRITE capability")
            return -13
        # In Wine translation, we call Lorifa VFS write
        fds = self.kernel.vfs.list_fds(self.session_id)
        if fd not in fds:
            # Fallback for simulated standard stdout/stderr
            if fd in [1, 2]:
                log_wine(f"Syscall WRITE stdout: writing mock buffer size {count} bytes.")
                return count
            return -9 # -EBADF
        
        # Call VFS write
        desc = fds[fd]
        payload = f"WineApp[PID:{self.process_id}] data payload"
        self.kernel.vfs.ramfs[desc.path].content = payload.encode("utf-8")
        log_wine(f"Syscall WRITE: Wrote via Lorifa VFS to '{desc.path}'")
        return count

    def sys_brk(self, new_brk):
        if new_brk == 0:
            return self.heap_brk
        if not self.kernel.check_capability(self.session_id, Capability.MEM_ALLOC):
            log_wine("Syscall BRK REJECTED: Lacks MEM_ALLOC capability")
            return -12
        if new_brk < self.heap_brk:
            return -22 # -EINVAL
        if new_brk > self.heap_limit:
            needed = new_brk - self.heap_limit
            log_wine(f"Syscall BRK: Heap extension needed. Requesting {needed} bytes from Lorifa VASM...")
            try:
                allocated_addr = self.kernel.allocate_memory(self.kernel.sessions[self.session_id].token.id, needed)
                self.heap_limit = allocated_addr + needed
                log_wine(f"Syscall BRK: Heap extended via Lorifa VASM. New limit: 0x{self.heap_limit:08X}")
            except Exception as e:
                log_alert(f"Syscall BRK allocation failed: {str(e)}")
                return -12 # -ENOMEM
        self.heap_brk = new_brk
        return self.heap_brk

    def sys_mmap(self, length):
        if not self.kernel.check_capability(self.session_id, Capability.MEM_ALLOC):
            log_wine("Syscall MMAP REJECTED: Lacks MEM_ALLOC capability")
            return -12
        try:
            log_wine(f"Syscall MMAP: Requesting {length} bytes from Lorifa VASM...")
            virtual_addr = self.kernel.allocate_memory(self.kernel.sessions[self.session_id].token.id, length)
            log_wine(f"Syscall MMAP: Mapped anonymous memory at 0x{virtual_addr:08X}")
            return virtual_addr
        except Exception as e:
            log_wine(f"Syscall MMAP failed: {str(e)}")
            return -12

    def sys_socket(self):
        if not self.kernel.check_capability(self.session_id, Capability.NET_CONNECT):
            log_wine("Syscall SOCKET REJECTED: Lacks NET_CONNECT capability")
            return -13
        fd = 100 + self.process_id
        log_wine(f"Syscall SOCKET: Created socket FD {fd}")
        return fd


def interactive_shell():
    kernel = LorifaKernelSimulation()
    
    current_token_id = kernel.create_token(
        "DefaultAdmin",
        PrivilegeLevel.ADMIN,
        parent_id=kernel.root_token_id,
        memory_limit=128 * 1024 * 1024,
        is_permanent=True,
        lifetime_seconds=999999,
        capabilities=0x1FF
    )
    
    current_session_id = kernel.run_process(current_token_id, session_lifetime=99999)

    print(f"\n{COLOR_KERNEL}========================================================================={COLOR_RESET}")
    print(f"{COLOR_KERNEL}              LORIFA KERNEL INTERACTIVE TEST TERMINAL                    {COLOR_RESET}")
    print(f"{COLOR_KERNEL}========================================================================={COLOR_RESET}")
    print("Welcome to Lorifa OS kernel debugging console.")
    print("Type 'help' to see the list of available commands.")
    print("Type 'exit' to quit.\n")

    while True:
        token = kernel.tokens.get(current_token_id)
        if not token:
            current_token_id = 2  # DefaultAdmin
            token = kernel.tokens[current_token_id]
            current_session_id = 1
            log_alert("\nActive Token was reclaimed due to expiration. Switched shell to DefaultAdmin.")
            
        prompt_char = "#" if token.privilege <= PrivilegeLevel.ADMIN else "$"
        prompt_color = COLOR_ALERT if token.privilege == PrivilegeLevel.ROOT else COLOR_APP
        
        prompt = f"{prompt_color}lorifa-{token.name.lower()}{prompt_char}{COLOR_RESET} "
        
        try:
            line = input(prompt).strip()
        except (KeyboardInterrupt, EOFError):
            print()
            break

        if not line:
            continue

        parts = line.split()
        cmd = parts[0].lower()
        args = parts[1:]

        if cmd == "exit":
            break
            
        elif cmd == "help":
            print("Available Commands:")
            print("  help                                       Show this help message")
            print("  status                                     Show Buddy Allocator memory state")
            print("  whoami                                     Show active shell token info")
            print("  escalate                                   Elevate current shell to Root privilege")
            print("  switch <token_id>                          Switch current shell to another Token")
            print("  token list                                 List all active tokens in the kernel")
            print("  token create <name> <privilege> <mem_kb>   Create a child token")
            print("  session list                               List all active sessions")
            print("  session start <token_id> <lifetime>        Create a new active process session")
            print("  alloc <token_id> <bytes>                   Allocate memory to a token (uses Buddy system)")
            print("  run-exe <session_id>                       Simulate a Windows app .exe call through Wine service")
            print("  scheduler list                             List all running threads inside the scheduler")
            print("  vfs open <path> <write_y_n>                [VFS] Open a file and get a File Descriptor")
            print("  vfs close <fd>                             [VFS] Close an open File Descriptor")
            print("  vfs read <fd> <bytes>                      [VFS] Read data from a file descriptor / driver")
            print("  vfs write <fd> <text>                      [VFS] Write data to a file descriptor / driver")
            print("  vfs files                                  [VFS] List all files present in RAMFS")
            print("  vfs list                                   [VFS] List active descriptors for active session")
            print("  ipc send <port_id> <message>               [IPC] Send a message, waking up any blocked thread")
            print("  ipc recv <port_id> <thread_id>             [IPC] Read message or block thread if empty")
            print("  compress <text>                            [LZ4] Compress text using lossless RLE engine")
            print("  decompress <hex>                           [LZ4] Decompress RLE hex data")
            print("  webview open <url>                         [WebKit] Spawn WebView container and render HTML")
            print("  driver list                                [Driver] List registered monolithic char devices")
            print("  tick <seconds>                             Advance the system clock to trigger timeouts")
            print("  exit                                       Quit the shell")
            
        elif cmd == "status":
            stat = kernel.ppa.get_status()
            print(f"--- Physical Page Allocator (Zig PPA) Status ---")
            print(f"  Total Memory:       {MEMORY_SIZE / (1024*1024):.1f} MB ({TOTAL_PAGES} pages)")
            print(f"  Allocated Memory:   {stat['allocated_bytes'] / 1024:.1f} KB ({stat['allocated_pages']} pages)")
            print(f"  Free Memory:        {stat['free_bytes'] / (1024*1024):.2f} MB ({stat['free_pages']} pages)")
            print(f"  Free blocks by Buddy Order:")
            for order, count in enumerate(stat["blocks_by_order"]):
                if count > 0:
                    pages = 1 << order
                    size_kb = (pages * PAGE_SIZE) / 1024
                    print(f"    Order {order:2d} ({size_kb:5.0f} KB blocks): {count} free block(s)")
                    
        elif cmd == "whoami":
            t = kernel.tokens[current_token_id]
            print(f"Active Token:       ID {t.id} ('{t.name}')")
            print(f"Privilege Level:    {PrivilegeLevel.to_string(t.privilege)}")
            print(f"Memory Allocation:  {t.memory_used / 1024:.1f} KB / {t.memory_limit / 1024:.1f} KB")
            print(f"Capabilities:       [{Capability.to_string(t.capabilities)}]")
            print(f"Remaining Time:     {t.expiry - kernel.current_time():.1f}s" if not t.is_permanent else "Permanent")
            print(f"Launch Count:       {t.run_count}/2 (Deprecated: {t.is_deprecated})")

        elif cmd == "escalate":
            if current_token_id == kernel.root_token_id:
                print("Already running under Root Token.")
            else:
                current_token_id = kernel.root_token_id
                current_session_id = 1
                print("Elevated to Root Token. Shell prompt changed to root system.")

        elif cmd == "switch":
            if not args:
                print("Error: switch requires <token_id>")
                continue
            try:
                target_id = int(args[0])
                if target_id not in kernel.tokens:
                    print(f"Error: Token {target_id} not found.")
                else:
                    current_token_id = target_id
                    print(f"Switched current shell to Token {target_id} ('{kernel.tokens[target_id].name}')")
            except ValueError:
                print("Error: Invalid Token ID.")

        elif cmd == "token" and len(args) > 0 and args[0] == "list":
            print(f"Active Kernel Tokens:")
            print(f"  {'ID':<3} | {'Name':<20} | {'Privilege':<10} | {'Memory (KB)':<15} | {'Lifespan':<10} | {'Runs':<4}")
            print(f"  " + "-"*75)
            for t_id, t in kernel.tokens.items():
                time_left = f"{t.expiry - kernel.current_time():.1f}s" if not t.is_permanent else "Permanent"
                mem = f"{t.memory_used/1024:.0f}/{t.memory_limit/1024:.0f}"
                print(f"  {t_id:<3} | {t.name:<20} | {PrivilegeLevel.to_string(t.privilege):<10} | {mem:<15} | {time_left:<10} | {t.run_count}/2")

        elif cmd == "token" and len(args) >= 6 and args[0] == "create":
            name = args[1]
            try:
                priv = PrivilegeLevel.parse(args[2])
                mem_limit = int(args[3]) * 1024
                lifetime = float(args[4])
                caps = Capability.parse_list(args[5])
                
                new_id = kernel.create_token(
                    name, priv, current_token_id, mem_limit, False, lifetime, caps
                )
                print(f"Token created successfully with ID {new_id}.")
            except Exception as e:
                print(f"Error creating token: {str(e)}")

        elif cmd == "session" and len(args) > 0 and args[0] == "list":
            print(f"Active Sessions:")
            print(f"  {'ID':<3} | {'Token ID':<8} | {'Token Name':<20} | {'Lifespan':<10} | {'State':<8}")
            print(f"  " + "-"*60)
            for s_id, s in kernel.sessions.items():
                time_left = f"{s.expiry - kernel.current_time():.1f}s"
                state = "ACTIVE" if s.is_active and s.expiry > kernel.current_time() else "EXPIRED"
                print(f"  {s_id:<3} | {s.token.id:<8} | {s.token.name:<20} | {time_left:<10} | {state:<8}")

        elif cmd == "session" and len(args) >= 3 and args[0] == "start":
            try:
                t_id = int(args[1])
                lifetime = float(args[2])
                new_s_id = kernel.run_process(t_id, lifetime)
                print(f"Session started successfully with ID {new_s_id}.")
            except Exception as e:
                print(f"Error starting session: {str(e)}")

        elif cmd == "alloc":
            if len(args) < 2:
                print("Usage: alloc <token_id> <bytes>")
                continue
            try:
                t_id = int(args[0])
                size = int(args[1])
                addr = kernel.allocate_memory(t_id, size)
                print(f"Allocated memory at virtual address: 0x{addr:08X}")
            except Exception as e:
                print(f"Allocation failed: {str(e)}")

        elif cmd == "run-exe":
            if not args:
                print("Usage: run-exe <session_id>")
                continue
            try:
                s_id = int(args[0])
                if s_id not in kernel.sessions:
                    print("Error: Session not found")
                    continue
                
                session = kernel.sessions[s_id]
                if not session.is_active or kernel.current_time() > session.expiry:
                    print("Error: Session is expired or inactive")
                    continue

                if not (session.token.capabilities & Capability.WINE_BRIDGE):
                    print("Permission Denied: Session token lacks WINE_BRIDGE capability (cannot boot Wine).")
                    continue

                translator = WineSyscallTranslator(kernel, s_id, process_id=600)
                print(f"\n--- Booting Wine container inside Session {s_id} ---")
                
                log_wine("Launching WINE loader container (ELF entry)...")
                kernel.scheduler.spawn(s_id, "WineLoaderMain", rip=0x30000000)
                
                translator.sys_brk(0)
                translator.sys_brk(0x40008000)
                translator.sys_mmap(4096 * 4)
                fd = translator.sys_open("/etc/motd", is_write=False)
                if fd >= 0:
                    translator.sys_write(fd, count=36)
                translator.sys_socket()
                
                log_wine("Notepad execution completed.")
                print(f"--------------------------------------------------\n")
            except Exception as e:
                print(f"Error executing wine translator: {str(e)}")

        elif cmd == "scheduler" and len(args) > 0 and args[0] == "list":
            print("Active Scheduler Threads (TCBs):")
            print(f"  {'TID':<3} | {'Session ID':<10} | {'Name':<20} | {'State':<8} | {'RIP':<12}")
            print(f"  " + "-"*60)
            for t in kernel.scheduler.threads:
                print(f"  {t.id:<3} | {t.session_id:<10} | {t.name:<20} | {t.state:<8} | 0x{t.rip:08X}")

        # VFS Commands
        elif cmd == "vfs" and len(args) >= 3 and args[0] == "open":
            path = args[1]
            is_write = args[2].lower() in ["y", "yes", "true", "w"]
            cap = Capability.FS_WRITE if is_write else Capability.FS_READ
            if kernel.check_capability(current_session_id, cap):
                if path.startswith("/dev/") and path not in kernel.driver_manager.drivers:
                    print(f"VFS Error: Device driver {path} not registered in Kernel.")
                else:
                    try:
                        fd = kernel.vfs.open(current_session_id, path, is_write)
                        print(f"VFS: File opened successfully. Assigned FD: {fd}")
                    except Exception as e:
                        print(f"VFS Error: {str(e)}")
            else:
                log_alert(f"VFS: Permission Denied. Lacks appropriate FS capabilities.")

        elif cmd == "vfs" and len(args) >= 2 and args[0] == "close":
            try:
                fd = int(args[1])
                if kernel.vfs.close(current_session_id, fd):
                    print("VFS: File descriptor closed successfully.")
                else:
                    print("VFS Error: FD not found in this session.")
            except ValueError:
                print("Error: Invalid File Descriptor.")

        elif cmd == "vfs" and len(args) >= 3 and args[0] == "read":
            try:
                fd = int(args[1])
                size = int(args[2])
                table = kernel.vfs.list_fds(current_session_id)
                if fd not in table:
                    print("Error: Invalid File Descriptor.")
                    continue
                desc = table[fd]
                if not kernel.check_capability(current_session_id, Capability.FS_READ):
                    log_alert("VFS read error: Lacks FS_READ capability.")
                    continue
                if desc.path.startswith("/dev/"):
                    data = kernel.driver_manager.read_device(desc.path, size)
                    if data is not None:
                        hex_repr = " ".join(f"{x:02x}" for x in data)
                        ascii_repr = "".join(chr(x) if 32 <= x <= 126 else "." for x in data)
                        print(f"VFS read: FD {fd} ({desc.path}) -> read {len(data)} bytes:")
                        print(f"  [HEX] {hex_repr}")
                        print(f"  [TXT] {ascii_repr}")
                    else:
                        print("Error: Device driver read failed.")
                else:
                    if desc.path in kernel.vfs.ramfs:
                        file_data = kernel.vfs.ramfs[desc.path].content
                        limit = min(size, len(file_data))
                        read_bytes = file_data[0:limit]
                        ascii_repr = read_bytes.decode("utf-8", errors="replace")
                        print(f"VFS read: FD {fd} ({desc.path}) -> read {limit} bytes: \"{ascii_repr}\"")
                    else:
                        print("Error: File not found in RAMFS.")
            except ValueError:
                print("Error: Invalid parameters. Usage: vfs read <fd> <bytes>")

        elif cmd == "vfs" and len(args) >= 3 and args[0] == "write":
            try:
                fd = int(args[1])
                payload = " ".join(args[2:])
                table = kernel.vfs.list_fds(current_session_id)
                if fd not in table:
                    print("Error: Invalid File Descriptor.")
                    continue
                desc = table[fd]
                if not desc.is_writeable:
                    print("Error: File descriptor is not writeable.")
                    continue
                if not kernel.check_capability(current_session_id, Capability.FS_WRITE):
                    log_alert("VFS write error: Lacks FS_WRITE capability.")
                    continue
                if desc.path.startswith("/dev/"):
                    bytes_written = kernel.driver_manager.write_device(desc.path, payload.encode("utf-8"))
                    print(f"VFS write: FD {fd} ({desc.path}) -> wrote {bytes_written} bytes.")
                else:
                    if desc.path in kernel.vfs.ramfs:
                        kernel.vfs.ramfs[desc.path].content = payload.encode("utf-8")
                        print(f"VFS write: FD {fd} ({desc.path}) -> wrote {len(payload)} bytes: \"{payload}\"")
                    else:
                        print("Error: File not found in RAMFS.")
            except ValueError:
                print("Error: Invalid parameters. Usage: vfs write <fd> <text>")

        elif cmd == "vfs" and len(args) > 0 and args[0] == "files":
            print("RAMFS Files:")
            print(f"  {'File Path':<30} | {'Size (Bytes)':<12}")
            print("  " + "-"*45)
            for path, file in kernel.vfs.ramfs.items():
                print(f"  {path:<30} | {len(file.content):<12}")

        elif cmd == "vfs" and len(args) > 0 and args[0] == "list":
            table = kernel.vfs.list_fds(current_session_id)
            print(f"Active File Descriptors for Session {current_session_id}:")
            print(f"  {'FD':<4} | {'Path':<30} | {'Access':<8}")
            print(f"  " + "-"*48)
            for fd, desc in table.items():
                access = "Writeable" if desc.is_writeable else "ReadOnly"
                print(f"  {fd:<4} | {desc.path:<30} | {access:<8}")

        # IPC Commands
        elif cmd == "ipc" and len(args) >= 3 and args[0] == "send":
            try:
                port_id = int(args[1])
                payload = " ".join(args[2:])
                woken = kernel.ipc.send(port_id, current_session_id, payload)
                if woken:
                    print(f"IPC: Message dispatched. Woke up blocked thread {woken}.")
                else:
                    print("IPC: Message dispatched to port queue.")
            except ValueError:
                print("Error: Invalid Port ID.")

        elif cmd == "ipc" and len(args) >= 3 and args[0] == "recv":
            try:
                port_id = int(args[1])
                thread_id = int(args[2])
                
                # Check if thread exists in scheduler
                thread_exists = any(t.id == thread_id for t in kernel.scheduler.threads)
                if not thread_exists:
                    print(f"Error: Thread ID {thread_id} not found in scheduler.")
                    continue
                
                msg = kernel.ipc.recv(port_id, thread_id)
                if msg:
                    print(f"IPC: Message received: \"{msg.payload}\" (from Session {msg.sender_session})")
                else:
                    print(f"IPC: No messages on Port {port_id}. Thread {thread_id} BLOCKED in scheduler.")
            except ValueError:
                print("Error: Invalid Port or Thread ID.")

        elif cmd == "compress":
            if not args:
                print("Usage: compress <text>")
                continue
            text = " ".join(args)
            data = text.encode("utf-8")
            compressed = []
            i = 0
            while i < len(data):
                run_len = 1
                while i + run_len < len(data) and data[i + run_len] == data[i] and run_len < 255:
                    run_len += 1
                compressed.append(run_len)
                compressed.append(data[i])
                i += run_len
            compressed_hex = "".join(f"{x:02x}" for x in compressed)
            ratio = (len(compressed) / len(data)) * 100
            print(f"[LZ4 Engine] Original size: {len(data)} bytes")
            print(f"[LZ4 Engine] Compressed size: {len(compressed)} bytes (Ratio: {ratio:.1f}%)")
            print(f"[LZ4 Engine] Hex payload: {compressed_hex}")

        elif cmd == "decompress":
            if not args:
                print("Usage: decompress <hex>")
                continue
            hex_str = args[0]
            try:
                compressed = bytes.fromhex(hex_str)
                if len(compressed) % 2 != 0:
                    print("Error: Invalid compressed payload format (odd length).")
                    continue
                decompressed = []
                i = 0
                while i < len(compressed):
                    count = compressed[i]
                    byte = compressed[i+1]
                    decompressed.extend([byte] * count)
                    i += 2
                decoded = bytes(decompressed).decode("utf-8")
                print(f"[LZ4 Engine] Decompressed text: \"{decoded}\"")
            except Exception as e:
                print(f"Error decompressing: {str(e)}")

        elif cmd == "webview" and len(args) >= 2 and args[0] == "open":
            url = args[1]
            if kernel.check_capability(current_session_id, Capability.MEM_ALLOC):
                fb_size = 1024 * 768 * 4
                fb_virtual_addr = 0xD0000000
                log_rust("WebKit: Spawning rendering task...")
                kernel.scheduler.spawn(current_session_id, "WebKitRenderTask", rip=0xD0000000)
                print(f"[WebKit] Allocating Framebuffer: {fb_size} bytes (~3MB). Address: 0x{fb_virtual_addr:08X}")
                print("\n+-------------------------------------------------------------+")
                print("| [WebKit WebView]                                            |")
                print("|                                                             |")
                if "google.com" in url:
                    print("|   Google Search Engine                                      |")
                    print("|   [ Search Input: ____________________ ] [ Search Button ]  |")
                elif "lorifa.org" in url:
                    print("|   Welcome to Lorifa Monolithic OS Project                   |")
                    print("|   Active core: Zig PPA + Rust VASM + WebKit webview         |")
                else:
                    print(f"|   Loading URL: {url:<45} |")
                    print("|   HTTP 200 OK: Content Loaded                               |")
                print("|                                                             |")
                print("+-------------------------------------------------------------+\n")
            else:
                log_alert("WebKit WebView: Failed to open. Lacks MEM_ALLOC capability.")

        elif cmd == "driver" and len(args) > 0 and args[0] == "list":
            print("Registered Monolithic Character Devices:")
            print(f"  {'Device Path':<15} | {'Driver Name':<15} | {'Operations':<20}")
            print("  " + "-" * 55)
            for path in kernel.driver_manager.drivers.keys():
                name = path.replace("/dev/", "").capitalize() + "Driver"
                ops = "read, write" if path == "/dev/fb0" or path == "/dev/urandom" else "write"
                if path == "/dev/null": ops = "read, write"
                print(f"  {path:<15} | {name:<15} | {ops:<20}")

        elif cmd == "tick":
            if not args:
                secs = 1.0
            else:
                try:
                    secs = float(args[0])
                except ValueError:
                    print("Error: Invalid duration")
                    continue

            print(f"Advancing kernel clock by {secs} seconds...")
            kernel.simulated_time_offset += secs
            kernel.tick()
            
        else:
            print(f"Command not found: {cmd}. Type 'help' for instructions.")


if __name__ == "__main__":
    interactive_shell()
