//! Event Chain - no_std implementation
//!
//! Fixed-capacity event chain for Rustacean OS kernel.

use super::{
    ChainableEvent, EventContext, EventMiddleware, FaultToleranceMode,
    result::{ChainResult, ChainStatus, EventFailure, EventResult, ErrorMessage},
};

/// Maximum number of events in a chain
const MAX_EVENTS: usize = 16;

/// Maximum number of middleware in a chain
const MAX_MIDDLEWARE: usize = 8;

/// Event chain orchestrator
///
/// Manages a pipeline of events with optional middleware.
/// Uses fixed-capacity arrays for no_std compatibility.
pub struct EventChain<'a> {
    /// Events to execute (stored as trait object references)
    events: [Option<&'a dyn ChainableEvent>; MAX_EVENTS],
    event_count: usize,
    
    /// Middleware stack (stored as trait object references)
    middleware: [Option<&'a dyn EventMiddleware>; MAX_MIDDLEWARE],
    middleware_count: usize,
    
    /// Fault tolerance mode
    fault_tolerance: FaultToleranceMode,
}

impl<'a> EventChain<'a> {
    /// Create a new empty event chain
    pub const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            event_count: 0,
            middleware: [None; MAX_MIDDLEWARE],
            middleware_count: 0,
            fault_tolerance: FaultToleranceMode::Strict,
        }
    }
    
    /// Set the fault tolerance mode
    pub fn with_fault_tolerance(mut self, mode: FaultToleranceMode) -> Self {
        self.fault_tolerance = mode;
        self
    }
    
    /// Add an event to the chain
    ///
    /// Events execute in FIFO order (first added = first executed).
    pub fn event(mut self, event: &'a dyn ChainableEvent) -> Self {
        if self.event_count < MAX_EVENTS {
            self.events[self.event_count] = Some(event);
            self.event_count += 1;
        }
        self
    }
    
    /// Add middleware to the chain
    ///
    /// Middleware executes in LIFO order (last added = first executed).
    pub fn middleware(mut self, mw: &'a dyn EventMiddleware) -> Self {
        if self.middleware_count < MAX_MIDDLEWARE {
            self.middleware[self.middleware_count] = Some(mw);
            self.middleware_count += 1;
        }
        self
    }
    
    /// Execute the event chain
    pub fn execute(&self, context: &mut EventContext) -> ChainResult {
        let mut result = ChainResult::success();
        let mut had_failures = false;
        
        for i in 0..self.event_count {
            let event = match self.events[i] {
                Some(e) => e,
                None => continue,
            };
            
            // Execute event with middleware pipeline
            let event_result = self.execute_with_middleware(event, context);
            
            if event_result.is_failure() {
                had_failures = true;
                
                let failure = EventFailure {
                    event_name: event.name(),
                    error: event_result.error_message()
                        .cloned()
                        .unwrap_or(ErrorMessage::from_static("unknown error")),
                    is_middleware_failure: event_result.is_middleware_failure(),
                };
                
                result.add_failure(failure);
                
                // Decide whether to continue based on fault tolerance
                match self.fault_tolerance {
                    FaultToleranceMode::Strict => {
                        result.success = false;
                        result.status = ChainStatus::Failed;
                        return result;
                    }
                    FaultToleranceMode::Lenient => {
                        // Continue regardless
                        continue;
                    }
                    FaultToleranceMode::BestEffort => {
                        if event_result.is_middleware_failure() {
                            // Stop on middleware failures
                            result.success = false;
                            result.status = ChainStatus::Failed;
                            return result;
                        }
                        // Continue on event failures
                        continue;
                    }
                }
            }
        }
        
        // Set final status
        if had_failures {
            result.status = ChainStatus::CompletedWithWarnings;
        }
        
        result
    }
    
    /// Execute a single event with the middleware pipeline
    fn execute_with_middleware(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
    ) -> EventResult<()> {
        if self.middleware_count == 0 {
            return event.execute(context);
        }
        
        // Build middleware chain (LIFO order)
        self.execute_middleware_recursive(0, event, context)
    }
    
    /// Recursively execute middleware stack
    fn execute_middleware_recursive(
        &self,
        index: usize,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
    ) -> EventResult<()> {
        if index >= self.middleware_count {
            // Base case: execute the actual event
            return event.execute(context);
        }
        
        // Get middleware in reverse order (LIFO)
        let mw_idx = self.middleware_count - 1 - index;
        let mw = match self.middleware[mw_idx] {
            Some(m) => m,
            None => return event.execute(context),
        };
        
        // Create next function
        let next = |ctx: &mut EventContext| -> EventResult<()> {
            self.execute_middleware_recursive(index + 1, event, ctx)
        };
        
        // Execute this middleware
        mw.execute(event, context, &next)
    }
}

impl<'a> Default for EventChain<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Static Event Chain (compile-time defined chains)
// ============================================================================

/// A statically-defined event chain using const generics
/// 
/// This allows defining chains at compile time with known event types,
/// avoiding dynamic dispatch overhead for hot paths.
pub struct StaticChain<E, const N: usize> {
    events: [E; N],
    fault_tolerance: FaultToleranceMode,
}

impl<E: ChainableEvent, const N: usize> StaticChain<E, N> {
    /// Create a new static chain from an array of events
    pub const fn new(events: [E; N]) -> Self {
        Self {
            events,
            fault_tolerance: FaultToleranceMode::Strict,
        }
    }
    
    /// Set fault tolerance mode
    pub const fn with_fault_tolerance(mut self, mode: FaultToleranceMode) -> Self {
        self.fault_tolerance = mode;
        self
    }
    
    /// Execute the chain (no middleware support for maximum performance)
    pub fn execute(&self, context: &mut EventContext) -> ChainResult {
        let mut result = ChainResult::success();
        let mut had_failures = false;
        
        for event in &self.events {
            let event_result = event.execute(context);
            
            if event_result.is_failure() {
                had_failures = true;
                
                let failure = EventFailure {
                    event_name: event.name(),
                    error: event_result.error_message()
                        .cloned()
                        .unwrap_or(ErrorMessage::from_static("unknown error")),
                    is_middleware_failure: event_result.is_middleware_failure(),
                };
                
                result.add_failure(failure);
                
                match self.fault_tolerance {
                    FaultToleranceMode::Strict => {
                        result.success = false;
                        result.status = ChainStatus::Failed;
                        return result;
                    }
                    _ => continue,
                }
            }
        }
        
        if had_failures {
            result.status = ChainStatus::CompletedWithWarnings;
        }
        
        result
    }
}
