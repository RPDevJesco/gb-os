//! Driver Initialization EventChain
//!
//! Uses EventChains to initialize hardware drivers in a fault-tolerant way.
//! Optional drivers (GPU, touchpad) can fail without stopping boot.
//! Required drivers (keyboard, basic display) must succeed.

use crate::event_chains::{
    ChainableEvent, EventChain, EventContext, EventMiddleware,
    FaultToleranceMode,
    result::EventResult,
    middleware::{LoggingMiddleware, NextHandler},
};

// =============================================================================
// Type Codes (since EventContext doesn't support strings)
// =============================================================================

/// GPU types
pub mod gpu_type {
    pub const UNKNOWN: u32 = 0;
    pub const ATI_RAGE: u32 = 1;
    pub const VESA: u32 = 2;
    pub const VGA_TEXT: u32 = 3;
}

/// Input types
pub mod input_type {
    pub const UNKNOWN: u32 = 0;
    pub const SYNAPTICS: u32 = 1;
    pub const PS2_VIA_SYNAPTICS: u32 = 2;
    pub const PS2_MOUSE: u32 = 3;
    pub const KEYBOARD_ONLY: u32 = 4;
}

// =============================================================================
// Context Keys
// =============================================================================

pub mod context_keys {
    // GPU
    pub const GPU_INITIALIZED: &str = "gpu_init";
    pub const GPU_TYPE: &str = "gpu_type";
    pub const FB_ADDR: &str = "fb_addr";
    pub const FB_WIDTH: &str = "fb_width";
    pub const FB_HEIGHT: &str = "fb_height";
    pub const FB_BPP: &str = "fb_bpp";
    pub const FB_PITCH: &str = "fb_pitch";
    pub const HW_CURSOR: &str = "hw_cursor";

    // Input
    pub const INPUT_INITIALIZED: &str = "input_init";
    pub const INPUT_TYPE: &str = "input_type";
    pub const KEYBOARD_INITIALIZED: &str = "kb_init";

    // Screen dimensions
    pub const SCREEN_WIDTH: &str = "scr_width";
    pub const SCREEN_HEIGHT: &str = "scr_height";

    // VESA fallback info
    pub const VESA_FB_ADDR: &str = "vesa_addr";
    pub const VESA_WIDTH: &str = "vesa_w";
    pub const VESA_HEIGHT: &str = "vesa_h";
    pub const VESA_BPP: &str = "vesa_bpp";
    pub const VESA_PITCH: &str = "vesa_pitch";
}

// =============================================================================
// Middleware: Dependency Checker
// =============================================================================

/// Middleware that checks driver dependencies before allowing execution
pub struct DependencyMiddleware;

impl DependencyMiddleware {
    pub const fn new() -> Self {
        Self
    }
}

impl EventMiddleware for DependencyMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: NextHandler<'_>,
    ) -> EventResult<()> {
        // Check dependencies based on event name
        match event.name() {
            // Synaptics/mouse requires screen dimensions
            "synaptics_init" | "ps2_mouse_init" => {
                if context.get_u32(context_keys::SCREEN_WIDTH).is_none() {
                    return EventResult::failure("Screen dimensions not set");
                }
            }
            _ => {}
        }

        next(context)
    }

    fn name(&self) -> &'static str {
        "DependencyMiddleware"
    }
}

// =============================================================================
// Driver Events
// =============================================================================

/// ATI Rage GPU Probe Event
pub struct AtiRageProbeEvent;

impl ChainableEvent for AtiRageProbeEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        match crate::drivers::ati_rage::init() {
            Ok(()) => {
                if let Some(gpu) = crate::drivers::ati_rage::get() {
                    let mode = crate::drivers::ati_rage::DisplayMode::mode_800x600_60();
                    match gpu.set_mode(&mode, 32) {
                        Ok(()) => {
                            context.set_bool(context_keys::GPU_INITIALIZED, true);
                            context.set_u32(context_keys::GPU_TYPE, gpu_type::ATI_RAGE);
                            context.set_u32(context_keys::FB_ADDR, gpu.framebuffer_addr());
                            context.set_u32(context_keys::FB_WIDTH, gpu.width());
                            context.set_u32(context_keys::FB_HEIGHT, gpu.height());
                            context.set_u32(context_keys::FB_BPP, gpu.bpp() / 8);
                            context.set_u32(context_keys::FB_PITCH, gpu.pitch());

                            gpu.enable_hw_cursor();
                            context.set_bool(context_keys::HW_CURSOR, true);

                            EventResult::success(())
                        }
                        Err(e) => EventResult::failure(e),
                    }
                } else {
                    EventResult::failure("GPU unavailable after init")
                }
            }
            Err(e) => EventResult::failure(e),
        }
    }

    fn name(&self) -> &'static str {
        "ati_rage_probe"
    }
}

/// VESA Fallback Event
pub struct VesaFallbackEvent;

impl ChainableEvent for VesaFallbackEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        // Skip if GPU already initialized
        if context.get_bool(context_keys::GPU_INITIALIZED).unwrap_or(false) {
            return EventResult::success(());
        }

        // Use VESA info passed from bootloader
        let fb_addr = match context.get_u32(context_keys::VESA_FB_ADDR) {
            Some(v) => v,
            None => return EventResult::failure("No VESA framebuffer"),
        };
        let width = match context.get_u32(context_keys::VESA_WIDTH) {
            Some(v) => v,
            None => return EventResult::failure("No VESA width"),
        };
        let height = match context.get_u32(context_keys::VESA_HEIGHT) {
            Some(v) => v,
            None => return EventResult::failure("No VESA height"),
        };
        let bpp = match context.get_u32(context_keys::VESA_BPP) {
            Some(v) => v,
            None => return EventResult::failure("No VESA bpp"),
        };
        let pitch = match context.get_u32(context_keys::VESA_PITCH) {
            Some(v) => v,
            None => return EventResult::failure("No VESA pitch"),
        };

        context.set_bool(context_keys::GPU_INITIALIZED, true);
        context.set_u32(context_keys::GPU_TYPE, gpu_type::VESA);
        context.set_u32(context_keys::FB_ADDR, fb_addr);
        context.set_u32(context_keys::FB_WIDTH, width);
        context.set_u32(context_keys::FB_HEIGHT, height);
        context.set_u32(context_keys::FB_BPP, bpp);
        context.set_u32(context_keys::FB_PITCH, pitch);
        context.set_bool(context_keys::HW_CURSOR, false);

        EventResult::success(())
    }

    fn name(&self) -> &'static str {
        "vesa_fallback"
    }
}

/// Framebuffer Init Event
pub struct FramebufferInitEvent;

impl ChainableEvent for FramebufferInitEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let fb_addr = match context.get_u32(context_keys::FB_ADDR) {
            Some(v) => v,
            None => return EventResult::failure("No framebuffer address"),
        };
        let width = match context.get_u32(context_keys::FB_WIDTH) {
            Some(v) => v,
            None => return EventResult::failure("No framebuffer width"),
        };
        let height = match context.get_u32(context_keys::FB_HEIGHT) {
            Some(v) => v,
            None => return EventResult::failure("No framebuffer height"),
        };
        let bpp = match context.get_u32(context_keys::FB_BPP) {
            Some(v) => v,
            None => return EventResult::failure("No framebuffer bpp"),
        };
        let pitch = match context.get_u32(context_keys::FB_PITCH) {
            Some(v) => v,
            None => return EventResult::failure("No framebuffer pitch"),
        };

        // Store screen dimensions for input drivers
        context.set_u32(context_keys::SCREEN_WIDTH, width);
        context.set_u32(context_keys::SCREEN_HEIGHT, height);

        unsafe {
            crate::gui::framebuffer::init(
                fb_addr as *mut u8,
                width,
                height,
                bpp,
                pitch,
            );
        }

        EventResult::success(())
    }

    fn name(&self) -> &'static str {
        "framebuffer_init"
    }
}

/// Synaptics Touchpad Init Event
pub struct SynapticsInitEvent;

impl ChainableEvent for SynapticsInitEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        if context.get_bool(context_keys::INPUT_INITIALIZED).unwrap_or(false) {
            return EventResult::success(());
        }

        let width = context.get_u32(context_keys::SCREEN_WIDTH).unwrap_or(800);
        let height = context.get_u32(context_keys::SCREEN_HEIGHT).unwrap_or(600);

        match crate::drivers::synaptics::init(width, height) {
            Ok(()) => {
                if crate::drivers::synaptics::is_synaptics() {
                    context.set_bool(context_keys::INPUT_INITIALIZED, true);
                    context.set_u32(context_keys::INPUT_TYPE, input_type::SYNAPTICS);
                } else {
                    context.set_bool(context_keys::INPUT_INITIALIZED, true);
                    context.set_u32(context_keys::INPUT_TYPE, input_type::PS2_VIA_SYNAPTICS);
                }
                EventResult::success(())
            }
            Err(e) => EventResult::failure(e),
        }
    }

    fn name(&self) -> &'static str {
        "synaptics_init"
    }
}

/// PS/2 Mouse Init Event (fallback)
pub struct Ps2MouseInitEvent;

impl ChainableEvent for Ps2MouseInitEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        if context.get_bool(context_keys::INPUT_INITIALIZED).unwrap_or(false) {
            return EventResult::success(());
        }

        let width = context.get_u32(context_keys::SCREEN_WIDTH).unwrap_or(800);
        let height = context.get_u32(context_keys::SCREEN_HEIGHT).unwrap_or(600);

        crate::drivers::mouse::init(width, height);

        context.set_bool(context_keys::INPUT_INITIALIZED, true);
        context.set_u32(context_keys::INPUT_TYPE, input_type::PS2_MOUSE);

        EventResult::success(())
    }

    fn name(&self) -> &'static str {
        "ps2_mouse_init"
    }
}

/// Keyboard Init Event
pub struct KeyboardInitEvent;

impl ChainableEvent for KeyboardInitEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        context.set_bool(context_keys::KEYBOARD_INITIALIZED, true);
        EventResult::success(())
    }

    fn name(&self) -> &'static str {
        "keyboard_init"
    }
}

// =============================================================================
// Global Event Instances
// =============================================================================

static ATI_RAGE_PROBE: AtiRageProbeEvent = AtiRageProbeEvent;
static VESA_FALLBACK: VesaFallbackEvent = VesaFallbackEvent;
static FRAMEBUFFER_INIT: FramebufferInitEvent = FramebufferInitEvent;
static SYNAPTICS_INIT: SynapticsInitEvent = SynapticsInitEvent;
static PS2_MOUSE_INIT: Ps2MouseInitEvent = Ps2MouseInitEvent;
static KEYBOARD_INIT: KeyboardInitEvent = KeyboardInitEvent;

static LOGGING_MW: LoggingMiddleware = LoggingMiddleware::new();
static DEPENDENCY_MW: DependencyMiddleware = DependencyMiddleware::new();

// =============================================================================
// Public API
// =============================================================================

/// Result of driver initialization
pub struct DriverInitResult {
    pub fb_addr: u32,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pitch: u32,
    pub gpu_type: u32,
    pub hw_cursor: bool,
    pub input_type: u32,
    pub failures: [Option<&'static str>; 8],
    pub failure_count: usize,
}

impl DriverInitResult {
    /// Check if using native ATI Rage driver
    pub fn is_ati_rage(&self) -> bool {
        self.gpu_type == gpu_type::ATI_RAGE
    }

    /// Check if using Synaptics touchpad
    pub fn is_synaptics(&self) -> bool {
        self.input_type == input_type::SYNAPTICS ||
            self.input_type == input_type::PS2_VIA_SYNAPTICS
    }

    /// Get GPU type as string (for display)
    pub fn gpu_type_str(&self) -> &'static str {
        match self.gpu_type {
            gpu_type::ATI_RAGE => "ATI Rage Mobility P",
            gpu_type::VESA => "VESA",
            gpu_type::VGA_TEXT => "VGA Text",
            _ => "Unknown",
        }
    }

    /// Get input type as string (for display)
    pub fn input_type_str(&self) -> &'static str {
        match self.input_type {
            input_type::SYNAPTICS => "Synaptics Touchpad",
            input_type::PS2_VIA_SYNAPTICS => "PS/2 Mouse (via Synaptics)",
            input_type::PS2_MOUSE => "PS/2 Mouse",
            input_type::KEYBOARD_ONLY => "Keyboard Only",
            _ => "Unknown",
        }
    }
}

/// Initialize all drivers using EventChain
pub fn init_all_drivers(
    vesa_fb_addr: u32,
    vesa_width: u32,
    vesa_height: u32,
    vesa_bpp: u32,
    vesa_pitch: u32,
) -> DriverInitResult {
    let mut context = EventContext::new();

    // Set VESA fallback info
    context.set_u32(context_keys::VESA_FB_ADDR, vesa_fb_addr);
    context.set_u32(context_keys::VESA_WIDTH, vesa_width);
    context.set_u32(context_keys::VESA_HEIGHT, vesa_height);
    context.set_u32(context_keys::VESA_BPP, vesa_bpp);
    context.set_u32(context_keys::VESA_PITCH, vesa_pitch);

    // Build driver init chain
    let chain = EventChain::new()
        .middleware(&LOGGING_MW)
        .middleware(&DEPENDENCY_MW)
        .event(&ATI_RAGE_PROBE)      // Try native GPU first
        .event(&VESA_FALLBACK)       // Fall back to VESA
        .event(&FRAMEBUFFER_INIT)    // Initialize framebuffer subsystem
        .event(&SYNAPTICS_INIT)      // Try Synaptics touchpad
        .event(&PS2_MOUSE_INIT)      // Fall back to PS/2 mouse
        .event(&KEYBOARD_INIT)       // Initialize keyboard
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let result = chain.execute(&mut context);

    // Collect failures
    let mut failures: [Option<&'static str>; 8] = [None; 8];
    let mut failure_count = 0;
    for failure in result.failures() {
        if failure_count < 8 {
            failures[failure_count] = Some(failure.event_name);
            failure_count += 1;
        }
    }

    // Extract results
    DriverInitResult {
        fb_addr: context.get_u32(context_keys::FB_ADDR).unwrap_or(vesa_fb_addr),
        width: context.get_u32(context_keys::FB_WIDTH).unwrap_or(vesa_width),
        height: context.get_u32(context_keys::FB_HEIGHT).unwrap_or(vesa_height),
        bpp: context.get_u32(context_keys::FB_BPP).unwrap_or(vesa_bpp),
        pitch: context.get_u32(context_keys::FB_PITCH).unwrap_or(vesa_pitch),
        gpu_type: context.get_u32(context_keys::GPU_TYPE).unwrap_or(gpu_type::UNKNOWN),
        hw_cursor: context.get_bool(context_keys::HW_CURSOR).unwrap_or(false),
        input_type: context.get_u32(context_keys::INPUT_TYPE).unwrap_or(input_type::UNKNOWN),
        failures,
        failure_count,
    }
}
