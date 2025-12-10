//! System Call Interface
//!
//! Rustacean OS system calls use EventChains for middleware support.
//! This allows adding logging, auditing, and permission checking to all syscalls.

use crate::event_chains::{
    ChainableEvent, EventChain, EventContext, FaultToleranceMode,
    result::EventResult,
    middleware::{LoggingMiddleware, PermissionMiddleware, AuditMiddleware},
};

/// System call numbers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SyscallNumber {
    /// Exit the current process
    Exit = 0,
    /// Read from file descriptor
    Read = 1,
    /// Write to file descriptor
    Write = 2,
    /// Open a file
    Open = 3,
    /// Close a file descriptor
    Close = 4,
    /// Get process ID
    GetPid = 5,
    /// Fork the current process
    Fork = 6,
    /// Execute a new program
    Exec = 7,
    /// Wait for child process
    Wait = 8,
    /// Create a pipe
    Pipe = 9,
    /// Memory map
    Mmap = 10,
    /// Memory unmap
    Munmap = 11,
    /// Yield CPU
    Yield = 12,
    /// Sleep for milliseconds
    Sleep = 13,
    /// Get current time
    Time = 14,
    /// Unknown syscall
    Unknown = 0xFFFFFFFF,
}

impl From<u32> for SyscallNumber {
    fn from(n: u32) -> Self {
        match n {
            0 => Self::Exit,
            1 => Self::Read,
            2 => Self::Write,
            3 => Self::Open,
            4 => Self::Close,
            5 => Self::GetPid,
            6 => Self::Fork,
            7 => Self::Exec,
            8 => Self::Wait,
            9 => Self::Pipe,
            10 => Self::Mmap,
            11 => Self::Munmap,
            12 => Self::Yield,
            13 => Self::Sleep,
            14 => Self::Time,
            _ => Self::Unknown,
        }
    }
}

/// System call parameters
#[derive(Debug, Clone, Copy)]
pub struct SyscallParams {
    pub number: SyscallNumber,
    pub arg1: u32,
    pub arg2: u32,
    pub arg3: u32,
    pub arg4: u32,
    pub arg5: u32,
}

impl SyscallParams {
    /// Create from register values
    pub fn from_regs(eax: u32, ebx: u32, ecx: u32, edx: u32, esi: u32, edi: u32) -> Self {
        Self {
            number: SyscallNumber::from(eax),
            arg1: ebx,
            arg2: ecx,
            arg3: edx,
            arg4: esi,
            arg5: edi,
        }
    }
}

// ============================================================================
// Syscall Events (for EventChains)
// ============================================================================

/// Exit syscall event
struct SyscallExit;

impl ChainableEvent for SyscallExit {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let exit_code = context.get_u32("arg1").unwrap_or(0);
        
        // Mark current task for termination
        // In a real implementation, this would interact with the scheduler
        
        context.set_u32("result", 0);
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_exit"
    }
}

/// Read syscall event
struct SyscallRead;

impl ChainableEvent for SyscallRead {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let fd = context.get_u32("arg1").unwrap_or(0);
        let buf = context.get_u32("arg2").unwrap_or(0);
        let count = context.get_u32("arg3").unwrap_or(0);
        
        // TODO: Implement file read
        // For now, just return 0 bytes read
        
        context.set_u32("result", 0);
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_read"
    }
}

/// Write syscall event
struct SyscallWrite;

impl ChainableEvent for SyscallWrite {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let fd = context.get_u32("arg1").unwrap_or(0);
        let buf = context.get_u32("arg2").unwrap_or(0);
        let count = context.get_u32("arg3").unwrap_or(0);
        
        // Handle stdout/stderr
        if fd == 1 || fd == 2 {
            // Write to console
            unsafe {
                let slice = core::slice::from_raw_parts(buf as *const u8, count as usize);
                if let Some(writer) = crate::drivers::vga::WRITER.as_mut() {
                    for &byte in slice {
                        writer.write_byte(byte);
                    }
                }
            }
            context.set_u32("result", count);
        } else {
            // TODO: Implement file write
            context.set_u32("result", 0);
        }
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_write"
    }
}

/// GetPid syscall event
struct SyscallGetPid;

impl ChainableEvent for SyscallGetPid {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        // Get current task's PID
        unsafe {
            if let Some(task) = crate::sched::SCHEDULER.current() {
                context.set_u32("result", (*task).pid);
            } else {
                context.set_u32("result", 0);
            }
        }
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_getpid"
    }
}

/// Yield syscall event
struct SyscallYield;

impl ChainableEvent for SyscallYield {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        crate::sched::schedule();
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_yield"
    }
}

/// Sleep syscall event
struct SyscallSleep;

impl ChainableEvent for SyscallSleep {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let ms = context.get_u32("arg1").unwrap_or(0);
        
        // TODO: Implement proper sleep with timer
        // For now, busy wait
        crate::arch::x86::pit::delay_ms(ms);
        
        context.set_u32("result", 0);
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_sleep"
    }
}

/// Time syscall event
struct SyscallTime;

impl ChainableEvent for SyscallTime {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let uptime = crate::arch::x86::pit::uptime_ms();
        context.set_u64("result64", uptime as u64);
        context.set_u32("result", uptime);
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "sys_time"
    }
}

/// Unknown syscall event
struct SyscallUnknown;

impl ChainableEvent for SyscallUnknown {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        context.set_u32("result", u32::MAX); // -1
        EventResult::failure("unknown syscall")
    }
    
    fn name(&self) -> &'static str {
        "sys_unknown"
    }
}

// ============================================================================
// Syscall Dispatch
// ============================================================================

/// Global middleware instances
static LOGGING_MW: LoggingMiddleware = LoggingMiddleware::new();
static PERMISSION_MW: PermissionMiddleware = PermissionMiddleware::user_allowed();
static AUDIT_MW: AuditMiddleware = AuditMiddleware::new();

/// Global syscall event instances
static SYSCALL_EXIT: SyscallExit = SyscallExit;
static SYSCALL_READ: SyscallRead = SyscallRead;
static SYSCALL_WRITE: SyscallWrite = SyscallWrite;
static SYSCALL_GETPID: SyscallGetPid = SyscallGetPid;
static SYSCALL_YIELD: SyscallYield = SyscallYield;
static SYSCALL_SLEEP: SyscallSleep = SyscallSleep;
static SYSCALL_TIME: SyscallTime = SyscallTime;
static SYSCALL_UNKNOWN: SyscallUnknown = SyscallUnknown;

/// Handle a system call
///
/// This is called from the interrupt handler for INT 0x80.
pub fn handle_syscall(params: SyscallParams) -> u32 {
    // Set up context with syscall parameters
    let mut context = EventContext::new();
    context.set_u32("syscall_number", params.number as u32);
    context.set_u32("arg1", params.arg1);
    context.set_u32("arg2", params.arg2);
    context.set_u32("arg3", params.arg3);
    context.set_u32("arg4", params.arg4);
    context.set_u32("arg5", params.arg5);
    context.set_u32("ring", 3); // User mode
    
    // Get the appropriate syscall event
    let event: &dyn ChainableEvent = match params.number {
        SyscallNumber::Exit => &SYSCALL_EXIT,
        SyscallNumber::Read => &SYSCALL_READ,
        SyscallNumber::Write => &SYSCALL_WRITE,
        SyscallNumber::GetPid => &SYSCALL_GETPID,
        SyscallNumber::Yield => &SYSCALL_YIELD,
        SyscallNumber::Sleep => &SYSCALL_SLEEP,
        SyscallNumber::Time => &SYSCALL_TIME,
        _ => &SYSCALL_UNKNOWN,
    };
    
    // Build the event chain with middleware
    let chain = EventChain::new()
        .middleware(&LOGGING_MW)
        .middleware(&PERMISSION_MW)
        .middleware(&AUDIT_MW)
        .event(event)
        .with_fault_tolerance(FaultToleranceMode::Strict);
    
    // Execute the chain
    let result = chain.execute(&mut context);
    
    // Return the result
    if result.success {
        context.get_u32("result").unwrap_or(0)
    } else {
        u32::MAX // -1 on error
    }
}

/// Initialize syscall handling
pub fn init() {
    // Set up INT 0x80 handler
    // This would be done in the IDT setup
    // For now, syscalls are dispatched manually
}
