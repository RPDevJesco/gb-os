//! Scheduler - Rustacean OS Process Scheduler
//!
//! Uses intrusive linked lists for run queues (no EventChains here - raw performance).
//! The scheduler is preemptive with priority-based round-robin.

use crate::mm::intrusive::{IntrusiveNode, IntrusiveQueue};
use core::sync::atomic::{AtomicU32, Ordering};

/// Process ID type
pub type Pid = u32;

/// Next available PID
static NEXT_PID: AtomicU32 = AtomicU32::new(1);

/// Allocate a new PID
fn alloc_pid() -> Pid {
    NEXT_PID.fetch_add(1, Ordering::Relaxed)
}

/// Task state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Task is ready to run
    Ready,
    /// Task is currently running
    Running,
    /// Task is blocked waiting for something
    Blocked,
    /// Task has exited
    Zombie,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    /// Idle priority (lowest)
    Idle = 0,
    /// Low priority
    Low = 1,
    /// Normal priority (default)
    Normal = 2,
    /// High priority
    High = 3,
    /// Real-time priority (highest)
    Realtime = 4,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Number of priority levels
const NUM_PRIORITIES: usize = 5;

/// Task Control Block
///
/// Contains all information about a task/process.
/// Uses intrusive nodes for zero-allocation queue management.
#[repr(C)]
pub struct Task {
    // Intrusive list nodes (must be first for simple offset calculation)
    /// Node for run queue linkage
    pub run_queue_node: IntrusiveNode,
    /// Node for wait queue linkage
    pub wait_queue_node: IntrusiveNode,
    
    // Task identification
    /// Process ID
    pub pid: Pid,
    /// Parent process ID
    pub ppid: Pid,
    /// Task name (for debugging)
    pub name: [u8; 16],
    
    // Scheduling info
    /// Current state
    pub state: TaskState,
    /// Priority level
    pub priority: Priority,
    /// Time slice remaining (in ticks)
    pub time_slice: u32,
    /// Total CPU time used (in ticks)
    pub cpu_time: u64,
    
    // CPU context (saved on context switch)
    /// Saved EAX
    pub eax: u32,
    /// Saved EBX
    pub ebx: u32,
    /// Saved ECX
    pub ecx: u32,
    /// Saved EDX
    pub edx: u32,
    /// Saved ESI
    pub esi: u32,
    /// Saved EDI
    pub edi: u32,
    /// Saved EBP
    pub ebp: u32,
    /// Saved ESP
    pub esp: u32,
    /// Saved EIP
    pub eip: u32,
    /// Saved EFLAGS
    pub eflags: u32,
    /// Saved CR3 (page directory)
    pub cr3: u32,
    
    // Memory info
    /// Kernel stack pointer
    pub kernel_stack: u32,
    /// User stack pointer
    pub user_stack: u32,
}

impl Task {
    /// Create a new task
    pub fn new(name: &str, priority: Priority) -> Self {
        let mut task = Self {
            run_queue_node: IntrusiveNode::new(),
            wait_queue_node: IntrusiveNode::new(),
            pid: alloc_pid(),
            ppid: 0,
            name: [0; 16],
            state: TaskState::Ready,
            priority,
            time_slice: 10, // 10 ticks = 100ms at 100Hz
            cpu_time: 0,
            eax: 0, ebx: 0, ecx: 0, edx: 0,
            esi: 0, edi: 0, ebp: 0, esp: 0,
            eip: 0, eflags: 0x202, // Interrupts enabled
            cr3: 0,
            kernel_stack: 0,
            user_stack: 0,
        };
        
        // Copy name
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(15);
        task.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        
        task
    }
    
    /// Get task name as string
    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&c| c == 0).unwrap_or(16);
        core::str::from_utf8(&self.name[..len]).unwrap_or("???")
    }
}

/// Multi-level feedback queue scheduler
pub struct Scheduler {
    /// Run queues for each priority level
    run_queues: [IntrusiveQueue<Task, fn(&Task) -> &IntrusiveNode>; NUM_PRIORITIES],
    /// Currently running task
    current: Option<*mut Task>,
    /// Idle task
    idle_task: Option<*mut Task>,
    /// Number of ready tasks
    ready_count: usize,
    /// Total context switches
    context_switches: u64,
}

/// Node accessor for run queue
fn run_queue_node(task: &Task) -> &IntrusiveNode {
    &task.run_queue_node
}

impl Scheduler {
    /// Create a new scheduler
    pub const fn new() -> Self {
        Self {
            run_queues: [
                IntrusiveQueue::new(run_queue_node),
                IntrusiveQueue::new(run_queue_node),
                IntrusiveQueue::new(run_queue_node),
                IntrusiveQueue::new(run_queue_node),
                IntrusiveQueue::new(run_queue_node),
            ],
            current: None,
            idle_task: None,
            ready_count: 0,
            context_switches: 0,
        }
    }
    
    /// Add a task to the run queue
    ///
    /// # Safety
    ///
    /// Task must remain valid and at a stable address while in the queue.
    pub unsafe fn enqueue(&mut self, task: &Task) {
        let priority = task.priority as usize;
        self.run_queues[priority].enqueue(task);
        self.ready_count += 1;
    }
    
    /// Pick the next task to run
    ///
    /// Returns the highest priority ready task.
    pub unsafe fn pick_next(&mut self) -> Option<*mut Task> {
        // Check queues from highest to lowest priority
        for priority in (0..NUM_PRIORITIES).rev() {
            if let Some(task) = self.run_queues[priority].dequeue() {
                self.ready_count -= 1;
                return Some(task.as_ptr());
            }
        }
        
        // No ready tasks, return idle task
        self.idle_task
    }
    
    /// Get the currently running task
    pub fn current(&self) -> Option<*mut Task> {
        self.current
    }
    
    /// Set the current task
    pub fn set_current(&mut self, task: Option<*mut Task>) {
        self.current = task;
    }
    
    /// Set the idle task
    pub fn set_idle(&mut self, task: *mut Task) {
        self.idle_task = Some(task);
    }
    
    /// Get the number of ready tasks
    pub fn ready_count(&self) -> usize {
        self.ready_count
    }
    
    /// Get total context switches
    pub fn context_switches(&self) -> u64 {
        self.context_switches
    }
    
    /// Increment context switch counter
    pub fn record_context_switch(&mut self) {
        self.context_switches += 1;
    }
    
    /// Called on timer tick
    ///
    /// Decrements current task's time slice and triggers reschedule if needed.
    pub unsafe fn timer_tick(&mut self) -> bool {
        if let Some(task) = self.current {
            let task = &mut *task;
            task.cpu_time += 1;
            
            if task.time_slice > 0 {
                task.time_slice -= 1;
            }
            
            // Need reschedule if time slice expired
            if task.time_slice == 0 {
                return true;
            }
        }
        
        false
    }
    
    /// Perform context switch
    ///
    /// # Safety
    ///
    /// Must be called with interrupts disabled.
    pub unsafe fn context_switch(old: *mut Task, new: *mut Task) {
        // Call the assembly implementation
        extern "C" {
            fn asm_context_switch(old: *mut Task, new: *mut Task);
        }
        asm_context_switch(old, new);
    }
}

// Context switch assembly implementation
core::arch::global_asm!(
    ".global asm_context_switch",
    "asm_context_switch:",
    // Save old task's registers
    "    mov eax, [esp + 4]",   // old task pointer
    "    mov [eax + 56], ebx",  // Save EBX (offset of ebx in Task)
    "    mov [eax + 60], ecx",  // Save ECX
    "    mov [eax + 64], edx",  // Save EDX
    "    mov [eax + 68], esi",  // Save ESI
    "    mov [eax + 72], edi",  // Save EDI
    "    mov [eax + 76], ebp",  // Save EBP
    "    mov [eax + 80], esp",  // Save ESP
    "    pushfd",
    "    pop dword ptr [eax + 88]",  // Save EFLAGS
    
    // Load new task's registers
    "    mov eax, [esp + 8]",   // new task pointer
    "    mov ebx, [eax + 56]",  // Load EBX
    "    mov ecx, [eax + 60]",  // Load ECX
    "    mov edx, [eax + 64]",  // Load EDX
    "    mov esi, [eax + 68]",  // Load ESI
    "    mov edi, [eax + 72]",  // Load EDI
    "    mov ebp, [eax + 76]",  // Load EBP
    "    mov esp, [eax + 80]",  // Load ESP
    "    push dword ptr [eax + 88]",
    "    popfd",                 // Load EFLAGS
    
    "    ret",
);

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Global scheduler instance
pub static mut SCHEDULER: Scheduler = Scheduler::new();

/// Initialize the scheduler
pub fn init() {
    // Scheduler is statically initialized, nothing to do here
    // The idle task will be created by the kernel after init
}

/// Called from timer interrupt
pub fn timer_tick() {
    unsafe {
        if SCHEDULER.timer_tick() {
            // Time slice expired, trigger reschedule
            schedule();
        }
    }
}

/// Trigger a reschedule
pub fn schedule() {
    unsafe {
        let old = SCHEDULER.current();
        
        // Put current task back in run queue if it's still runnable
        if let Some(old_ptr) = old {
            let old_task = &*old_ptr;
            if old_task.state == TaskState::Running {
                // Reset time slice and re-enqueue
                (*old_ptr).state = TaskState::Ready;
                (*old_ptr).time_slice = 10;
                SCHEDULER.enqueue(&*old_ptr);
            }
        }
        
        // Pick next task
        if let Some(new_ptr) = SCHEDULER.pick_next() {
            (*new_ptr).state = TaskState::Running;
            SCHEDULER.set_current(Some(new_ptr));
            SCHEDULER.record_context_switch();
            
            if let Some(old_ptr) = old {
                if old_ptr != new_ptr {
                    Scheduler::context_switch(old_ptr, new_ptr);
                }
            }
        }
    }
}
