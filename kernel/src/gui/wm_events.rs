//! Window Manager EventChain
//!
//! Handles discrete window lifecycle events through EventChains:
//! - Window creation/destruction
//! - Focus changes
//! - Z-order changes (bring to front/send to back)
//! - Window move/resize completion
//!
//! NOTE: Continuous events (mouse tracking, frame rendering) stay outside
//! EventChains for performance reasons. Only discrete, state-changing
//! events go through the chain.

use crate::event_chains::{
    ChainableEvent, EventChain, EventContext, EventMiddleware,
    FaultToleranceMode,
    result::EventResult,
    middleware::{LoggingMiddleware, NextHandler},
};

// =============================================================================
// Window Event Types
// =============================================================================

/// Window event type codes
pub mod event_type {
    pub const WINDOW_CREATE: u32 = 1;
    pub const WINDOW_DESTROY: u32 = 2;
    pub const FOCUS_CHANGE: u32 = 3;
    pub const Z_ORDER_CHANGE: u32 = 4;
    pub const WINDOW_MOVE: u32 = 5;
    pub const WINDOW_RESIZE: u32 = 6;
}

/// Z-order change directions
pub mod z_order {
    pub const BRING_TO_FRONT: u32 = 1;
    pub const SEND_TO_BACK: u32 = 2;
    pub const MOVE_UP: u32 = 3;
    pub const MOVE_DOWN: u32 = 4;
}

// =============================================================================
// Context Keys
// =============================================================================

pub mod context_keys {
    // Event identification
    pub const EVENT_TYPE: &str = "wm_event";
    pub const WINDOW_ID: &str = "wm_win_id";
    
    // Window creation
    pub const WIN_TITLE_LEN: &str = "wm_title_len";
    pub const WIN_X: &str = "wm_x";
    pub const WIN_Y: &str = "wm_y";
    pub const WIN_WIDTH: &str = "wm_width";
    pub const WIN_HEIGHT: &str = "wm_height";
    
    // Focus
    pub const OLD_FOCUS: &str = "wm_old_focus";
    pub const NEW_FOCUS: &str = "wm_new_focus";
    
    // Z-order
    pub const Z_DIRECTION: &str = "wm_z_dir";
    pub const OLD_Z_INDEX: &str = "wm_old_z";
    pub const NEW_Z_INDEX: &str = "wm_new_z";
    
    // Move/resize
    pub const OLD_X: &str = "wm_old_x";
    pub const OLD_Y: &str = "wm_old_y";
    pub const NEW_X: &str = "wm_new_x";
    pub const NEW_Y: &str = "wm_new_y";
    pub const OLD_WIDTH: &str = "wm_old_w";
    pub const OLD_HEIGHT: &str = "wm_old_h";
    pub const NEW_WIDTH: &str = "wm_new_w";
    pub const NEW_HEIGHT: &str = "wm_new_h";
    
    // Result
    pub const SUCCESS: &str = "wm_success";
    pub const RESULT_WINDOW_ID: &str = "wm_result_id";
}

// =============================================================================
// Middleware: Focus Policy
// =============================================================================

/// Middleware that enforces focus policies
/// 
/// For example: preventing certain windows from stealing focus,
/// or requiring user interaction before focus change.
pub struct FocusPolicyMiddleware {
    /// Allow focus stealing (window requesting focus without user click)
    allow_focus_steal: bool,
}

impl FocusPolicyMiddleware {
    pub const fn new() -> Self {
        Self {
            allow_focus_steal: true, // Permissive by default
        }
    }
    
    pub const fn strict() -> Self {
        Self {
            allow_focus_steal: false,
        }
    }
}

impl EventMiddleware for FocusPolicyMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Check focus policy for focus change events
        if event.name() == "wm_focus_change" && !self.allow_focus_steal {
            // In strict mode, we could check if this focus change was
            // initiated by user interaction vs programmatic request
            // For now, we allow all focus changes
        }
        
        next(context)
    }
    
    fn name(&self) -> &'static str {
        "FocusPolicyMiddleware"
    }
}

// =============================================================================
// Middleware: Audit Trail
// =============================================================================

/// Middleware that logs window management operations
/// 
/// Useful for debugging and for implementing "recent windows" features.
pub struct WmAuditMiddleware {
    // In a real implementation, this would write to a ring buffer
    // of recent window operations
}

impl WmAuditMiddleware {
    pub const fn new() -> Self {
        Self {}
    }
}

impl EventMiddleware for WmAuditMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Log before execution
        let _event_type = context.get_u32(context_keys::EVENT_TYPE).unwrap_or(0);
        let _window_id = context.get_u32(context_keys::WINDOW_ID).unwrap_or(0);
        
        // In a real implementation:
        // audit_log.push(AuditEntry { event_type, window_id, timestamp });
        
        let result = next(context);
        
        // Log after execution (success/failure)
        // audit_log.last_mut().set_result(result.is_success());
        
        result
    }
    
    fn name(&self) -> &'static str {
        "WmAuditMiddleware"
    }
}

// =============================================================================
// Window Events
// =============================================================================

/// Window Creation Event
/// 
/// Called when a new window is being created.
/// Validates parameters and allocates window slot.
pub struct WindowCreateEvent;

impl ChainableEvent for WindowCreateEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let x = context.get_u32(context_keys::WIN_X).unwrap_or(50) as i32;
        let y = context.get_u32(context_keys::WIN_Y).unwrap_or(50) as i32;
        let width = context.get_u32(context_keys::WIN_WIDTH).unwrap_or(400);
        let height = context.get_u32(context_keys::WIN_HEIGHT).unwrap_or(300);
        
        // Validate dimensions
        if width < 100 || height < 50 {
            return EventResult::failure("Window too small");
        }
        if width > 2000 || height > 2000 {
            return EventResult::failure("Window too large");
        }
        
        // The actual window creation is done by the caller after the event succeeds
        // We just validate and prepare here
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_window_create"
    }
}

/// Window Destruction Event
/// 
/// Called when a window is being destroyed.
pub struct WindowDestroyEvent;

impl ChainableEvent for WindowDestroyEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let window_id = match context.get_u32(context_keys::WINDOW_ID) {
            Some(id) => id,
            None => return EventResult::failure("No window ID specified"),
        };
        
        // Validate window exists (would check desktop.windows in real impl)
        if window_id == 0 {
            return EventResult::failure("Invalid window ID");
        }
        
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_window_destroy"
    }
}

/// Focus Change Event
/// 
/// Called when window focus is changing.
pub struct FocusChangeEvent;

impl ChainableEvent for FocusChangeEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let _old_focus = context.get_u32(context_keys::OLD_FOCUS);
        let new_focus = context.get_u32(context_keys::NEW_FOCUS);
        
        // Validate new focus target exists
        if new_focus.is_none() {
            // Clearing focus (clicking desktop) is valid
            context.set_bool(context_keys::SUCCESS, true);
            return EventResult::success(());
        }
        
        // In a real implementation, verify the window exists
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_focus_change"
    }
}

/// Z-Order Change Event
/// 
/// Called when a window's z-order is changing (bring to front, etc).
pub struct ZOrderChangeEvent;

impl ChainableEvent for ZOrderChangeEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let window_id = match context.get_u32(context_keys::WINDOW_ID) {
            Some(id) => id,
            None => return EventResult::failure("No window ID specified"),
        };
        
        let direction = context.get_u32(context_keys::Z_DIRECTION)
            .unwrap_or(z_order::BRING_TO_FRONT);
        
        // Validate direction
        if direction < 1 || direction > 4 {
            return EventResult::failure("Invalid z-order direction");
        }
        
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_z_order_change"
    }
}

/// Window Move Event
/// 
/// Called when a window move operation completes (drag released).
/// NOT called during dragging - that's handled directly for performance.
pub struct WindowMoveEvent;

impl ChainableEvent for WindowMoveEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let window_id = match context.get_u32(context_keys::WINDOW_ID) {
            Some(id) => id,
            None => return EventResult::failure("No window ID specified"),
        };
        
        let new_x = context.get_u32(context_keys::NEW_X);
        let new_y = context.get_u32(context_keys::NEW_Y);
        
        if new_x.is_none() || new_y.is_none() {
            return EventResult::failure("No new position specified");
        }
        
        // Could validate that window stays on screen
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_window_move"
    }
}

/// Window Resize Event
/// 
/// Called when a window resize operation completes.
pub struct WindowResizeEvent;

impl ChainableEvent for WindowResizeEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let window_id = match context.get_u32(context_keys::WINDOW_ID) {
            Some(id) => id,
            None => return EventResult::failure("No window ID specified"),
        };
        
        let new_width = context.get_u32(context_keys::NEW_WIDTH).unwrap_or(100);
        let new_height = context.get_u32(context_keys::NEW_HEIGHT).unwrap_or(50);
        
        // Validate minimum size
        if new_width < 100 || new_height < 50 {
            return EventResult::failure("Window too small");
        }
        
        context.set_bool(context_keys::SUCCESS, true);
        
        EventResult::success(())
    }
    
    fn name(&self) -> &'static str {
        "wm_window_resize"
    }
}

// =============================================================================
// Global Instances
// =============================================================================

static WINDOW_CREATE: WindowCreateEvent = WindowCreateEvent;
static WINDOW_DESTROY: WindowDestroyEvent = WindowDestroyEvent;
static FOCUS_CHANGE: FocusChangeEvent = FocusChangeEvent;
static Z_ORDER_CHANGE: ZOrderChangeEvent = ZOrderChangeEvent;
static WINDOW_MOVE: WindowMoveEvent = WindowMoveEvent;
static WINDOW_RESIZE: WindowResizeEvent = WindowResizeEvent;

static LOGGING_MW: LoggingMiddleware = LoggingMiddleware::new();
static FOCUS_POLICY_MW: FocusPolicyMiddleware = FocusPolicyMiddleware::new();
static AUDIT_MW: WmAuditMiddleware = WmAuditMiddleware::new();

// =============================================================================
// Public API
// =============================================================================

/// Window Manager EventChain handler
/// 
/// Call these methods from Desktop to dispatch events through the chain.
pub struct WmEventDispatcher;

impl WmEventDispatcher {
    /// Dispatch a window creation event
    /// Returns true if creation should proceed
    pub fn dispatch_create(x: i32, y: i32, width: u32, height: u32) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::WINDOW_CREATE);
        context.set_u32(context_keys::WIN_X, x as u32);
        context.set_u32(context_keys::WIN_Y, y as u32);
        context.set_u32(context_keys::WIN_WIDTH, width);
        context.set_u32(context_keys::WIN_HEIGHT, height);
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&AUDIT_MW)
            .event(&WINDOW_CREATE)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
    
    /// Dispatch a window destruction event
    /// Returns true if destruction should proceed
    pub fn dispatch_destroy(window_id: u32) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::WINDOW_DESTROY);
        context.set_u32(context_keys::WINDOW_ID, window_id);
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&AUDIT_MW)
            .event(&WINDOW_DESTROY)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
    
    /// Dispatch a focus change event
    /// Returns true if focus change should proceed
    pub fn dispatch_focus_change(old_focus: Option<u32>, new_focus: Option<u32>) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::FOCUS_CHANGE);
        
        if let Some(old) = old_focus {
            context.set_u32(context_keys::OLD_FOCUS, old);
        }
        if let Some(new) = new_focus {
            context.set_u32(context_keys::NEW_FOCUS, new);
        }
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&FOCUS_POLICY_MW)
            .middleware(&AUDIT_MW)
            .event(&FOCUS_CHANGE)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
    
    /// Dispatch a z-order change event
    /// Returns true if z-order change should proceed
    pub fn dispatch_z_order_change(window_id: u32, direction: u32) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::Z_ORDER_CHANGE);
        context.set_u32(context_keys::WINDOW_ID, window_id);
        context.set_u32(context_keys::Z_DIRECTION, direction);
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&AUDIT_MW)
            .event(&Z_ORDER_CHANGE)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
    
    /// Dispatch a window move completion event
    /// Returns true if the move is valid
    pub fn dispatch_move(window_id: u32, old_x: i32, old_y: i32, new_x: i32, new_y: i32) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::WINDOW_MOVE);
        context.set_u32(context_keys::WINDOW_ID, window_id);
        context.set_u32(context_keys::OLD_X, old_x as u32);
        context.set_u32(context_keys::OLD_Y, old_y as u32);
        context.set_u32(context_keys::NEW_X, new_x as u32);
        context.set_u32(context_keys::NEW_Y, new_y as u32);
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&AUDIT_MW)
            .event(&WINDOW_MOVE)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
    
    /// Dispatch a window resize completion event
    /// Returns true if the resize is valid
    pub fn dispatch_resize(
        window_id: u32, 
        old_w: u32, old_h: u32, 
        new_w: u32, new_h: u32
    ) -> bool {
        let mut context = EventContext::new();
        context.set_u32(context_keys::EVENT_TYPE, event_type::WINDOW_RESIZE);
        context.set_u32(context_keys::WINDOW_ID, window_id);
        context.set_u32(context_keys::OLD_WIDTH, old_w);
        context.set_u32(context_keys::OLD_HEIGHT, old_h);
        context.set_u32(context_keys::NEW_WIDTH, new_w);
        context.set_u32(context_keys::NEW_HEIGHT, new_h);
        
        let chain = EventChain::new()
            .middleware(&LOGGING_MW)
            .middleware(&AUDIT_MW)
            .event(&WINDOW_RESIZE)
            .with_fault_tolerance(FaultToleranceMode::Strict);
        
        let result = chain.execute(&mut context);
        result.success
    }
}
