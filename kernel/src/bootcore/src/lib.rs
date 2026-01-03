//! # bootcore
//!
//! Shared abstractions for the rustboot unified bootloader.
//! This crate provides platform-agnostic traits and types that all
//! platform implementations must conform to.

#![no_std]
#![feature(const_trait_impl)]

pub mod traits;
pub mod boot_info;
pub mod fmt;
pub mod panic;

pub use traits::*;
pub use boot_info::BootInfo;
