//! Event Middleware - no_std implementation
//!
//! Middleware for cross-cutting concerns in Rustacean OS kernel.

use super::{ChainableEvent, EventContext, result::EventResult};

/// Next handler function type (non-generic for object safety)
pub type NextHandler<'a> = &'a dyn Fn(&mut EventContext) -> EventResult<()>;

/// Middleware trait
///
/// Middleware wraps around event execution to add cross-cutting concerns
/// like logging, permissions, auditing, timing, etc.
pub trait EventMiddleware {
    /// Execute the middleware
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()>;
    
    /// Name of this middleware (for debugging)
    fn name(&self) -> &'static str;
}

// ============================================================================
// Built-in Middleware
// ============================================================================

/// Logging middleware - logs event execution
pub struct LoggingMiddleware {
    log_success: bool,
    log_failure: bool,
}

impl LoggingMiddleware {
    pub const fn new() -> Self {
        Self {
            log_success: true,
            log_failure: true,
        }
    }
    
    pub const fn errors_only() -> Self {
        Self {
            log_success: false,
            log_failure: true,
        }
    }
}

impl EventMiddleware for LoggingMiddleware {
    fn execute(
        &self,
        _event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        let result = next(context);
        // In a real implementation, we'd log here
        result
    }
    
    fn name(&self) -> &'static str {
        "LoggingMiddleware"
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Permission checking middleware
pub struct PermissionMiddleware {
    required_ring: u8,
}

impl PermissionMiddleware {
    pub const fn kernel_only() -> Self {
        Self { required_ring: 0 }
    }
    
    pub const fn user_allowed() -> Self {
        Self { required_ring: 3 }
    }
    
    pub const fn new(ring: u8) -> Self {
        Self { required_ring: ring }
    }
}

impl EventMiddleware for PermissionMiddleware {
    fn execute(
        &self,
        _event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Check permission level from context
        let current_ring = context.get_u32("ring").unwrap_or(0) as u8;
        
        if current_ring > self.required_ring {
            return EventResult::failure("insufficient privileges");
        }
        
        next(context)
    }
    
    fn name(&self) -> &'static str {
        "PermissionMiddleware"
    }
}

impl Default for PermissionMiddleware {
    fn default() -> Self {
        Self::kernel_only()
    }
}

/// Audit logging middleware
pub struct AuditMiddleware;

impl AuditMiddleware {
    pub const fn new() -> Self {
        Self
    }
}

impl EventMiddleware for AuditMiddleware {
    fn execute(
        &self,
        _event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Record audit entry (timestamp, user, event)
        // For now, just pass through
        next(context)
    }
    
    fn name(&self) -> &'static str {
        "AuditMiddleware"
    }
}

impl Default for AuditMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Timing middleware - measures execution time
pub struct TimingMiddleware;

impl TimingMiddleware {
    pub const fn new() -> Self {
        Self
    }
}

impl EventMiddleware for TimingMiddleware {
    fn execute(
        &self,
        _event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Would use PIT ticks in a real implementation
        let result = next(context);
        result
    }
    
    fn name(&self) -> &'static str {
        "TimingMiddleware"
    }
}

impl Default for TimingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry middleware - retries failed events
pub struct RetryMiddleware {
    max_retries: u8,
}

impl RetryMiddleware {
    pub const fn new(max_retries: u8) -> Self {
        Self { max_retries }
    }
    
    pub const fn default_retries() -> Self {
        Self { max_retries: 3 }
    }
}

impl EventMiddleware for RetryMiddleware {
    fn execute(
        &self,
        _event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        let mut last_result = EventResult::failure("no attempts made");
        
        for _ in 0..=self.max_retries {
            last_result = next(context);
            if last_result.is_success() {
                return last_result;
            }
        }
        
        last_result
    }
    
    fn name(&self) -> &'static str {
        "RetryMiddleware"
    }
}

impl Default for RetryMiddleware {
    fn default() -> Self {
        Self::default_retries()
    }
}
