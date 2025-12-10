//! mkgamedisk - GameBoy ROM to Floppy Image Converter
//!
//! Creates a floppy disk image containing a GameBoy ROM for use with GameBoy OS.
//!
//! # Usage
//! ```
//! mkgamedisk <input.gb> <output.img>
//! ```
//!
//! # Game Floppy Format
//!
//! ```text
//! Sector 0 (512 bytes): Header
//!   Offset 0x00: Magic "GBOY" (4 bytes)
//!   Offset 0x04: ROM size in bytes (4 bytes, little-endian)
//!   Offset 0x08: ROM title (32 bytes, null-padded)
//!   Offset 0x28: Reserved (472 bytes)
//!
//! Sectors 1+: Raw GameBoy ROM data
//! ```
//!
//! Maximum ROM size: 1,474,048 bytes (2879 sectors * 512 - 512 header)
//! Fits most GameBoy games (Pokemon Red/Blue is ~1MB)

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::Path;

/// Floppy disk size (1.44MB)
const FLOPPY_SIZE: usize = 1474560;

/// Sector size
const SECTOR_SIZE: usize = 512;

/// Maximum ROM size (floppy minus header sector)
const MAX_ROM_SIZE: usize = FLOPPY_SIZE - SECTOR_SIZE;

/// Magic bytes: "GBOY"
const MAGIC: &[u8; 4] = b"GBOY";

/// GameBoy ROM header offsets
const GB_TITLE_START: usize = 0x134;
const GB_TITLE_END: usize = 0x143;
const GB_CGB_FLAG: usize = 0x143;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("mkgamedisk - GameBoy ROM to Floppy Image Converter");
        eprintln!();
        eprintln!("Usage: {} <input.gb> <output.img>", args[0]);
        eprintln!();
        eprintln!("Creates a floppy disk image containing the GameBoy ROM");
        eprintln!("for use with GameBoy OS.");
        eprintln!();
        eprintln!("Maximum ROM size: {} bytes ({:.2} MB)",
                  MAX_ROM_SIZE, MAX_ROM_SIZE as f64 / 1024.0 / 1024.0);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    // Read input ROM
    let rom_data = fs::read(input_path)?;
    let rom_size = rom_data.len();

    println!("Input ROM: {}", input_path);
    println!("ROM size: {} bytes ({} KB)", rom_size, rom_size / 1024);

    // Validate ROM size
    if rom_size > MAX_ROM_SIZE {
        eprintln!("Error: ROM too large! Maximum size is {} bytes", MAX_ROM_SIZE);
        std::process::exit(1);
    }

    if rom_size < 0x150 {
        eprintln!("Error: ROM too small to be a valid GameBoy ROM");
        std::process::exit(1);
    }

    // Extract title from ROM header
    let title = extract_title(&rom_data);
    println!("ROM title: {}", title);

    // Detect GameBoy Color
    let is_cgb = rom_data[GB_CGB_FLAG] & 0x80 != 0;
    println!("Type: {}", if is_cgb { "GameBoy Color" } else { "GameBoy" });

    // Create floppy image
    let mut image = vec![0u8; FLOPPY_SIZE];

    // Write header (sector 0)
    // Magic
    image[0..4].copy_from_slice(MAGIC);

    // ROM size (little-endian u32)
    let size_bytes = (rom_size as u32).to_le_bytes();
    image[4..8].copy_from_slice(&size_bytes);

    // Title (32 bytes, null-padded)
    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len().min(31);
    image[8..8 + title_len].copy_from_slice(&title_bytes[..title_len]);
    // Rest is already zeros

    // Write ROM data (starting at sector 1)
    image[SECTOR_SIZE..SECTOR_SIZE + rom_size].copy_from_slice(&rom_data);

    // Write output file
    let mut output = File::create(output_path)?;
    output.write_all(&image)?;

    println!();
    println!("Created: {}", output_path);
    println!("Image size: {} bytes (1.44MB floppy)", FLOPPY_SIZE);

    // Calculate sectors used
    let sectors_used = (rom_size + SECTOR_SIZE - 1) / SECTOR_SIZE + 1;
    println!("Sectors used: {} / 2880", sectors_used);

    // Instructions
    println!();
    println!("To write to a physical floppy disk:");
    println!("  Linux:   dd if={} of=/dev/fd0 bs=512", output_path);
    println!("  Windows: Use RawWrite or similar tool");

    Ok(())
}

/// Extract game title from ROM header
fn extract_title(rom: &[u8]) -> String {
    if rom.len() < GB_TITLE_END {
        return String::from("Unknown");
    }

    // CGB flag at 0x143 indicates shorter title (11 bytes vs 16)
    let title_end = if rom[GB_CGB_FLAG] & 0x80 != 0 {
        GB_CGB_FLAG // 11 bytes: 0x134-0x142
    } else {
        GB_TITLE_END + 1 // 16 bytes: 0x134-0x143
    };

    let title_bytes = &rom[GB_TITLE_START..title_end];

    // Find null terminator or end
    let len = title_bytes.iter()
        .position(|&b| b == 0)
        .unwrap_or(title_bytes.len());

    // Convert to string, filtering non-printable characters
    title_bytes[..len]
        .iter()
        .filter(|&&b| b >= 0x20 && b < 0x7F)
        .map(|&b| b as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        // Minimal valid ROM with title "TEST"
        let mut rom = vec![0u8; 0x150];
        rom[0x134] = b'T';
        rom[0x135] = b'E';
        rom[0x136] = b'S';
        rom[0x137] = b'T';
        rom[0x138] = 0; // Null terminator

        assert_eq!(extract_title(&rom), "TEST");
    }

    #[test]
    fn test_cgb_title() {
        let mut rom = vec![0u8; 0x150];
        rom[0x134..0x13F].copy_from_slice(b"POKEMON RED");
        rom[0x143] = 0x80; // CGB flag

        assert_eq!(extract_title(&rom), "POKEMON RED");
    }
}
