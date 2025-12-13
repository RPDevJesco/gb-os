//! FAT16 Filesystem Reader
//!
//! Read-only FAT16 filesystem support for loading ROMs from the ROMS partition.
//! This is a minimal implementation focused on reading .gb and .gbc files.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::ata::{Ata, AtaDrive};

/// FAT16 Boot Sector / BIOS Parameter Block
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Fat16Bpb {
    pub jump: [u8; 3],
    pub oem_id: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media_type: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    // Extended BPB
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

/// FAT16 Directory Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub name: [u8; 8],
    pub ext: [u8; 3],
    pub attr: u8,
    pub reserved: u8,
    pub create_time_tenths: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub access_date: u16,
    pub first_cluster_hi: u16,
    pub modify_time: u16,
    pub modify_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

/// Directory entry attributes
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE: u8 = 0x20;
pub const ATTR_LONG_NAME: u8 = 0x0F;

/// End of cluster chain marker
pub const FAT_EOC: u16 = 0xFFF8;

/// ROM file information
#[derive(Debug, Clone)]
pub struct RomFile {
    pub name: String,
    pub size: u32,
    pub first_cluster: u16,
}

/// FAT16 filesystem reader
pub struct Fat16 {
    ata: Ata,
    partition_start: u32,

    // Cached BPB values
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    root_entries: u16,
    fat_size: u16,

    // Calculated values
    fat_start: u32,
    root_start: u32,
    data_start: u32,

    // Sector buffer
    sector_buffer: [u8; 512],
}

impl Fat16 {
    /// Create a new FAT16 filesystem reader
    ///
    /// # Arguments
    /// * `drive` - ATA drive to read from
    /// * `partition_start` - LBA of partition start
    pub fn new(drive: AtaDrive, partition_start: u32) -> Result<Self, &'static str> {
        let mut fs = Self {
            ata: Ata::new(drive),
            partition_start,
            bytes_per_sector: 512,
            sectors_per_cluster: 1,
            reserved_sectors: 1,
            num_fats: 2,
            root_entries: 512,
            fat_size: 0,
            fat_start: 0,
            root_start: 0,
            data_start: 0,
            sector_buffer: [0; 512],
        };

        fs.read_bpb()?;
        Ok(fs)
    }

    /// Read and parse the BPB (BIOS Parameter Block)
    fn read_bpb(&mut self) -> Result<(), &'static str> {
        // Read boot sector
        self.ata.read_sectors(self.partition_start, 1, &mut self.sector_buffer)?;

        // Parse BPB
        let bpb = unsafe { &*(self.sector_buffer.as_ptr() as *const Fat16Bpb) };

        // Validate
        if bpb.bytes_per_sector != 512 {
            return Err("Unsupported sector size");
        }

        // Check filesystem type
        let fs_type = core::str::from_utf8(&bpb.fs_type).unwrap_or("");
        if !fs_type.starts_with("FAT16") && !fs_type.starts_with("FAT") {
            return Err("Not a FAT16 filesystem");
        }

        // Store values
        self.bytes_per_sector = bpb.bytes_per_sector;
        self.sectors_per_cluster = bpb.sectors_per_cluster;
        self.reserved_sectors = bpb.reserved_sectors;
        self.num_fats = bpb.num_fats;
        self.root_entries = bpb.root_entries;
        self.fat_size = bpb.fat_size_16;

        // Calculate region starts
        self.fat_start = self.partition_start + self.reserved_sectors as u32;
        self.root_start = self.fat_start + (self.num_fats as u32 * self.fat_size as u32);

        let root_sectors = ((self.root_entries as u32 * 32) + 511) / 512;
        self.data_start = self.root_start + root_sectors;

        Ok(())
    }

    /// List ROM files (.gb and .gbc) in root directory
    pub fn list_roms(&mut self) -> Result<Vec<RomFile>, &'static str> {
        let mut roms = Vec::new();
        let root_sectors = ((self.root_entries as u32 * 32) + 511) / 512;

        for sector_idx in 0..root_sectors {
            self.ata.read_sectors(self.root_start + sector_idx, 1, &mut self.sector_buffer)?;

            // Each sector has 16 directory entries (512 / 32)
            for i in 0..16 {
                let offset = i * 32;
                let entry = unsafe {
                    &*(self.sector_buffer.as_ptr().add(offset) as *const DirEntry)
                };

                // Check for end of directory
                if entry.name[0] == 0x00 {
                    return Ok(roms);
                }

                // Skip deleted entries
                if entry.name[0] == 0xE5 {
                    continue;
                }

                // Skip long name entries, directories, volume labels
                if entry.attr & ATTR_LONG_NAME == ATTR_LONG_NAME {
                    continue;
                }
                if entry.attr & (ATTR_DIRECTORY | ATTR_VOLUME_ID) != 0 {
                    continue;
                }

                // Check extension
                let ext = core::str::from_utf8(&entry.ext).unwrap_or("").trim();
                let is_rom = ext.eq_ignore_ascii_case("GB") || ext.eq_ignore_ascii_case("GBC");

                if is_rom {
                    // Build filename
                    let name_part = core::str::from_utf8(&entry.name)
                        .unwrap_or("")
                        .trim();

                    let mut name = String::from(name_part);
                    name.push('.');
                    name.push_str(ext);

                    roms.push(RomFile {
                        name,
                        size: entry.file_size,
                        first_cluster: entry.first_cluster_lo,
                    });
                }
            }
        }

        Ok(roms)
    }

    /// Read a ROM file into memory
    ///
    /// # Arguments
    /// * `rom` - ROM file info from list_roms()
    /// * `dest` - Destination address to load ROM
    ///
    /// # Safety
    /// Caller must ensure dest points to valid memory with enough space
    pub unsafe fn load_rom(&mut self, rom: &RomFile, dest: *mut u8) -> Result<u32, &'static str> {
        let mut cluster = rom.first_cluster;
        let mut bytes_read: u32 = 0;
        let mut dest_ptr = dest;

        let cluster_size = self.sectors_per_cluster as u32 * 512;

        while cluster >= 2 && cluster < FAT_EOC {
            // Calculate LBA for this cluster
            let cluster_lba = self.data_start +
                ((cluster as u32 - 2) * self.sectors_per_cluster as u32);

            // Read cluster
            for sector in 0..self.sectors_per_cluster {
                self.ata.read_sectors(
                    cluster_lba + sector as u32,
                    1,
                    &mut self.sector_buffer
                )?;

                // Copy to destination
                let bytes_to_copy = core::cmp::min(
                    512,
                    (rom.size - bytes_read) as usize
                );

                core::ptr::copy_nonoverlapping(
                    self.sector_buffer.as_ptr(),
                    dest_ptr,
                    bytes_to_copy
                );

                dest_ptr = dest_ptr.add(512);
                bytes_read += 512;

                if bytes_read >= rom.size {
                    return Ok(rom.size);
                }
            }

            // Get next cluster from FAT
            cluster = self.get_next_cluster(cluster)?;
        }

        Ok(bytes_read.min(rom.size))
    }

    /// Get next cluster from FAT table
    fn get_next_cluster(&mut self, cluster: u16) -> Result<u16, &'static str> {
        // Each FAT16 entry is 2 bytes
        let fat_offset = cluster as u32 * 2;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let entry_offset = (fat_offset % 512) as usize;

        self.ata.read_sectors(fat_sector, 1, &mut self.sector_buffer)?;

        let next = u16::from_le_bytes([
            self.sector_buffer[entry_offset],
            self.sector_buffer[entry_offset + 1],
        ]);

        Ok(next)
    }

    /// Get total size of filesystem in bytes
    pub fn total_size(&self) -> u64 {
        // This would need total_sectors from BPB
        0
    }
}

/// Parse MBR partition table and find ROMS partition
///
/// Returns the LBA of the ROMS partition (partition 2, type 0x06/0x0E for FAT16)
pub fn find_roms_partition(ata: &mut Ata) -> Result<u32, &'static str> {
    let mut buffer = [0u8; 512];

    // Read MBR at LBA 0
    ata.read_sectors(0, 1, &mut buffer)?;

    // Check MBR signature
    if buffer[510] != 0x55 || buffer[511] != 0xAA {
        return Err("Invalid MBR signature");
    }

    // Partition table starts at offset 0x1BE
    // Each entry is 16 bytes, we want partition 2 (index 1)
    let part2_offset = 0x1BE + 16; // Second partition entry

    let part_type = buffer[part2_offset + 4];

    // FAT16 partition types: 0x04, 0x06, 0x0E, 0x14, 0x16, 0x1E
    let is_fat16 = matches!(part_type, 0x04 | 0x06 | 0x0E | 0x14 | 0x16 | 0x1E);

    if !is_fat16 {
        return Err("Partition 2 is not FAT16");
    }

    // Get starting LBA (little-endian at offset 8 within entry)
    let lba = u32::from_le_bytes([
        buffer[part2_offset + 8],
        buffer[part2_offset + 9],
        buffer[part2_offset + 10],
        buffer[part2_offset + 11],
    ]);

    if lba == 0 {
        return Err("Partition 2 not found");
    }

    Ok(lba)
}
