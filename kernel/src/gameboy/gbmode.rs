//! GameBoy Mode Definitions

/// GameBoy hardware mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbMode {
    /// Original GameBoy (DMG)
    Classic,
    /// GameBoy Color
    Color,
    /// GameBoy Color running a classic game
    ColorAsClassic,
}

/// CPU speed mode (CGB only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbSpeed {
    /// Normal speed (4.19 MHz)
    Single = 1,
    /// Double speed (8.38 MHz, CGB only)
    Double = 2,
}
