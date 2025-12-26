//! UEFI Module
//! 
//! Complete UEFI definitions without external dependencies.

pub mod types;
pub mod tables;
pub mod protocols;

pub use types::*;
pub use tables::*;
pub use protocols::*;
