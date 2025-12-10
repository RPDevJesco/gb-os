//! Kernel EventChains - no_std Implementation
//!
//! A stripped-down version of EventChains for use in the kernel.
//! This version uses:
//! - Fixed-capacity arrays instead of Vec
//! - Static dispatch where possible
//! - No heap allocation
//!
//! # Architecture
//!
//! EventChains in Rustacean OS are used for:
//! - Kernel syscall processing (with permission/audit middleware)
//! - Window manager events (focus, damage, layout)
//! - GUI layer events (input validation, theming)
//!
//! NOT used for (raw performance needed):
//! - Memory allocator
//! - Scheduler run queues
//! - Interrupt handlers (before dispatch)

pub mod context;
pub mod result;
pub mod chain;
pub mod middleware;

// Re-exports
pub use context::EventContext;
pub use result::EventResult;
pub use chain::EventChain;
pub use middleware::EventMiddleware;

/// Trait for chainable events
pub trait ChainableEvent {
    /// Execute the event with the given context
    fn execute(&self, context: &mut EventContext) -> EventResult<()>;
    
    /// Get the name of this event (for logging/debugging)
    fn name(&self) -> &'static str;
}

/// Fault tolerance mode for event chains
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultToleranceMode {
    /// Stop on any failure
    Strict,
    /// Continue on event failures, collect for review
    Lenient,
    /// Continue on event failures, stop on middleware failures
    BestEffort,
}

impl Default for FaultToleranceMode {
    fn default() -> Self {
        Self::Strict
    }
}
