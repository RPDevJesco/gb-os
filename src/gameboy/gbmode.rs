//! Game Boy mode and speed definitions

/// Game Boy hardware mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbMode {
    /// Original Game Boy (DMG)
    Classic,
    /// Game Boy Color (CGB)
    Color,
    /// Color hardware running a Classic game
    ColorAsClassic,
}

/// CPU clock speed mode (CGB only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbSpeed {
    /// Normal speed (1x, ~4.19 MHz)
    Single = 1,
    /// Double speed (2x, ~8.38 MHz, CGB only)
    Double = 2,
}

impl Default for GbMode {
    fn default() -> Self {
        Self::Classic
    }
}

impl Default for GbSpeed {
    fn default() -> Self {
        Self::Single
    }
}
