//! FAT32 Filesystem Implementation
//!
//! A minimal FAT32 driver for reading files from SD card.
//! Supports Long Filenames (LFN) for ROM enumeration.
//!
//! Features:
//! - MBR partition table parsing
//! - FAT32 boot sector parsing
//! - Stateful directory enumeration
//! - Long Filename (LFN) support
//! - File reading by cluster chain
//!
//! Limitations:
//! - Read-only
//! - Root directory only (no subdirectory traversal)

use crate::drivers::sdhost::{SdCard, SECTOR_SIZE};

// ============================================================================
// Constants
// ============================================================================

/// End of cluster chain marker (minimum value)
const FAT32_EOC_MIN: u32 = 0x0FFF_FFF8;

/// Directory entry size in bytes
const DIR_ENTRY_SIZE: usize = 32;

/// Entries per sector
const ENTRIES_PER_SECTOR: usize = SECTOR_SIZE / DIR_ENTRY_SIZE;

/// Maximum filename length we support (LFN can be up to 255 UTF-16 chars)
pub const MAX_FILENAME_LEN: usize = 128;

/// Characters per LFN entry
const LFN_CHARS_PER_ENTRY: usize = 13;

// ============================================================================
// Directory Entry Attributes
// ============================================================================

pub mod attr {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    /// LFN entry marker (READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID)
    pub const LONG_NAME: u8 = 0x0F;
    pub const LONG_NAME_MASK: u8 = 0x3F;
}

// ============================================================================
// ROM Entry - Result of enumeration
// ============================================================================

/// Information about a ROM file
#[derive(Clone, Copy)]
pub struct RomEntry {
    /// Filename buffer (ASCII, from LFN or 8.3)
    pub name: [u8; MAX_FILENAME_LEN],
    /// Actual length of filename
    pub name_len: usize,
    /// First cluster of file data
    pub cluster: u32,
    /// File size in bytes
    pub size: u32,
    /// True if .gbc extension (Game Boy Color)
    pub is_gbc: bool,
}

impl RomEntry {
    /// Create an empty ROM entry
    pub const fn empty() -> Self {
        Self {
            name: [0u8; MAX_FILENAME_LEN],
            name_len: 0,
            cluster: 0,
            size: 0,
            is_gbc: false,
        }
    }

    /// Get filename as a string slice
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("<invalid>")
    }
}

// ============================================================================
// Directory Enumerator State
// ============================================================================

/// State for iterating through directory entries
pub struct DirEnumerator {
    /// Current cluster being scanned
    cluster: u32,
    /// Current sector within cluster (0..sectors_per_cluster)
    sector_in_cluster: u8,
    /// Current entry within sector (0..ENTRIES_PER_SECTOR)
    entry_in_sector: usize,
    /// Cached current sector data
    sector_data: [u8; SECTOR_SIZE],
    /// Whether sector_data is valid
    sector_loaded: bool,
    /// Reached end of directory
    finished: bool,

    // LFN accumulation state
    /// LFN buffer (UTF-16 code units)
    lfn_buffer: [u16; 256],
    /// Number of valid UTF-16 chars in lfn_buffer
    lfn_len: usize,
    /// Expected LFN sequence number (counting down)
    lfn_seq_expected: u8,
    /// LFN checksum for validation
    lfn_checksum: u8,
    /// Whether we have a valid accumulated LFN
    lfn_valid: bool,
}

impl DirEnumerator {
    /// Create a new enumerator starting at the given cluster
    pub fn new(root_cluster: u32) -> Self {
        Self {
            cluster: root_cluster,
            sector_in_cluster: 0,
            entry_in_sector: 0,
            sector_data: [0u8; SECTOR_SIZE],
            sector_loaded: false,
            finished: false,
            lfn_buffer: [0u16; 256],
            lfn_len: 0,
            lfn_seq_expected: 0,
            lfn_checksum: 0,
            lfn_valid: false,
        }
    }

    /// Reset to beginning of directory
    pub fn reset(&mut self, root_cluster: u32) {
        self.cluster = root_cluster;
        self.sector_in_cluster = 0;
        self.entry_in_sector = 0;
        self.sector_loaded = false;
        self.finished = false;
        self.clear_lfn();
    }

    /// Clear accumulated LFN state
    fn clear_lfn(&mut self) {
        self.lfn_len = 0;
        self.lfn_seq_expected = 0;
        self.lfn_checksum = 0;
        self.lfn_valid = false;
    }

    /// Calculate 8.3 filename checksum for LFN validation
    fn calc_checksum(name_8_3: &[u8; 11]) -> u8 {
        let mut sum: u8 = 0;
        for &b in name_8_3 {
            sum = sum.rotate_right(1).wrapping_add(b);
        }
        sum
    }

    /// Process an LFN entry, accumulating characters
    fn process_lfn_entry(&mut self, entry: &[u8]) {
        let order = entry[0];
        let checksum = entry[13];

        // Check for start of new LFN sequence (bit 6 set = last entry, which comes first)
        if order & 0x40 != 0 {
            self.clear_lfn();
            self.lfn_seq_expected = order & 0x1F;
            self.lfn_checksum = checksum;
        }

        let seq = order & 0x1F;

        // Validate sequence
        if seq != self.lfn_seq_expected || checksum != self.lfn_checksum {
            self.clear_lfn();
            return;
        }

        // Calculate offset in buffer (entries come in reverse order)
        let char_offset = ((seq - 1) as usize) * LFN_CHARS_PER_ENTRY;

        // Extract UTF-16 characters from entry
        // Positions: 1-10 (5 chars), 14-25 (6 chars), 28-31 (2 chars)
        let chars: [u16; 13] = [
            u16::from_le_bytes([entry[1], entry[2]]),
            u16::from_le_bytes([entry[3], entry[4]]),
            u16::from_le_bytes([entry[5], entry[6]]),
            u16::from_le_bytes([entry[7], entry[8]]),
            u16::from_le_bytes([entry[9], entry[10]]),
            u16::from_le_bytes([entry[14], entry[15]]),
            u16::from_le_bytes([entry[16], entry[17]]),
            u16::from_le_bytes([entry[18], entry[19]]),
            u16::from_le_bytes([entry[20], entry[21]]),
            u16::from_le_bytes([entry[22], entry[23]]),
            u16::from_le_bytes([entry[24], entry[25]]),
            u16::from_le_bytes([entry[28], entry[29]]),
            u16::from_le_bytes([entry[30], entry[31]]),
        ];

        // Copy to buffer, stopping at null terminator
        for (i, &ch) in chars.iter().enumerate() {
            if ch == 0x0000 || ch == 0xFFFF {
                // End of name in this entry
                if char_offset + i > self.lfn_len {
                    self.lfn_len = char_offset + i;
                }
                break;
            }
            if char_offset + i < self.lfn_buffer.len() {
                self.lfn_buffer[char_offset + i] = ch;
                if char_offset + i + 1 > self.lfn_len {
                    self.lfn_len = char_offset + i + 1;
                }
            }
        }

        // Move to next expected sequence number
        self.lfn_seq_expected = seq - 1;

        // If we've received all entries (seq == 1), mark LFN as complete
        if seq == 1 {
            self.lfn_valid = true;
        }
    }

    /// Convert accumulated LFN (UTF-16) to ASCII in output buffer
    fn copy_lfn_to_entry(&self, entry: &mut RomEntry) {
        let mut out_len = 0;
        for i in 0..self.lfn_len {
            if out_len >= MAX_FILENAME_LEN {
                break;
            }
            let ch = self.lfn_buffer[i];
            // Simple UTF-16 to ASCII: keep ASCII range, replace others with '?'
            entry.name[out_len] = if ch > 0 && ch < 128 {
                ch as u8
            } else {
                b'?'
            };
            out_len += 1;
        }
        entry.name_len = out_len;
    }

    /// Convert 8.3 name to readable format in output buffer
    fn copy_8_3_to_entry(dir_entry: &[u8], entry: &mut RomEntry) {
        let mut out_len = 0;

        // Copy name part (8 bytes), trimming trailing spaces
        let mut name_end = 8;
        while name_end > 0 && dir_entry[name_end - 1] == b' ' {
            name_end -= 1;
        }
        for i in 0..name_end {
            entry.name[out_len] = dir_entry[i];
            out_len += 1;
        }

        // Add dot
        entry.name[out_len] = b'.';
        out_len += 1;

        // Copy extension (3 bytes), trimming trailing spaces
        let mut ext_end = 3;
        while ext_end > 0 && dir_entry[8 + ext_end - 1] == b' ' {
            ext_end -= 1;
        }
        for i in 0..ext_end {
            entry.name[out_len] = dir_entry[8 + i];
            out_len += 1;
        }

        entry.name_len = out_len;
    }
}

// ============================================================================
// FAT32 Filesystem
// ============================================================================

/// FAT32 filesystem driver
pub struct Fat32 {
    /// Underlying SD card driver
    sd: SdCard,
    /// Filesystem is mounted
    mounted: bool,
    /// First sector of FAT
    fat_start_sector: u32,
    /// First sector of data area
    data_start_sector: u32,
    /// Root directory cluster
    root_cluster: u32,
    /// Sectors per cluster
    sectors_per_cluster: u8,
    /// Bytes per sector (usually 512)
    bytes_per_sector: u32,
}

impl Fat32 {
    /// Create a new FAT32 filesystem instance
    pub const fn new() -> Self {
        Self {
            sd: SdCard::new(),
            mounted: false,
            fat_start_sector: 0,
            data_start_sector: 0,
            root_cluster: 0,
            sectors_per_cluster: 0,
            bytes_per_sector: SECTOR_SIZE as u32,
        }
    }

    /// Check if filesystem is mounted
    pub fn is_mounted(&self) -> bool {
        self.mounted
    }

    /// Get root cluster (for creating enumerators)
    pub fn root_cluster(&self) -> u32 {
        self.root_cluster
    }

    /// Get sectors per cluster
    pub fn sectors_per_cluster(&self) -> u8 {
        self.sectors_per_cluster
    }

    /// Mount the filesystem
    pub fn mount(&mut self) -> Result<(), &'static str> {
        // Initialize SD card
        self.sd.init()?;

        let mut sector = [0u8; SECTOR_SIZE];

        // Read MBR (sector 0)
        self.sd.read_sector(0, &mut sector)?;

        // Check MBR signature
        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid MBR signature");
        }

        // Get first partition start sector
        let part_start = u32::from_le_bytes([
            sector[0x1BE + 8],
            sector[0x1BE + 9],
            sector[0x1BE + 10],
            sector[0x1BE + 11],
        ]);

        // Read VBR
        self.sd.read_sector(part_start, &mut sector)?;

        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid VBR signature");
        }

        // Parse BPB
        self.bytes_per_sector = u16::from_le_bytes([sector[11], sector[12]]) as u32;
        self.sectors_per_cluster = sector[13];
        let reserved_sectors = u16::from_le_bytes([sector[14], sector[15]]) as u32;
        let num_fats = sector[16] as u32;
        let fat_size = u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]);
        self.root_cluster = u32::from_le_bytes([sector[44], sector[45], sector[46], sector[47]]);

        self.fat_start_sector = part_start + reserved_sectors;
        self.data_start_sector = self.fat_start_sector + (num_fats * fat_size);

        self.mounted = true;
        Ok(())
    }

    /// Convert cluster number to first sector number
    fn cluster_to_sector(&self, cluster: u32) -> u32 {
        let cluster_offset = cluster - 2;
        self.data_start_sector + (cluster_offset * self.sectors_per_cluster as u32)
    }

    /// Get the next cluster in a chain from the FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let entry_offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut sector = [0u8; SECTOR_SIZE];
        self.sd.read_sector(fat_sector, &mut sector)?;

        let next = u32::from_le_bytes([
            sector[entry_offset],
            sector[entry_offset + 1],
            sector[entry_offset + 2],
            sector[entry_offset + 3],
        ]) & 0x0FFF_FFFF;

        Ok(next)
    }

    /// Check if cluster indicates end of chain
    fn is_end_of_chain(cluster: u32) -> bool {
        cluster < 2 || cluster >= FAT32_EOC_MIN
    }

    /// Check if directory entry has ROM extension (.gb or .gbc)
    fn is_rom_extension(entry: &[u8]) -> bool {
        let ext0 = entry[8].to_ascii_uppercase();
        let ext1 = entry[9].to_ascii_uppercase();
        let ext2 = entry[10].to_ascii_uppercase();

        ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C')
    }

    /// Check if extension is .GBC (Game Boy Color)
    fn is_gbc_extension(entry: &[u8]) -> bool {
        entry[8].to_ascii_uppercase() == b'G'
            && entry[9].to_ascii_uppercase() == b'B'
            && entry[10].to_ascii_uppercase() == b'C'
    }

    /// Create a new directory enumerator
    pub fn enumerate_roms(&self) -> DirEnumerator {
        DirEnumerator::new(self.root_cluster)
    }

    /// Get the next ROM entry using the given enumerator
    ///
    /// Returns `true` if a ROM was found and `entry` was populated,
    /// `false` if no more ROMs exist.
    /// Get the next ROM entry using the given enumerator
    pub fn next_rom(&mut self, enum_state: &mut DirEnumerator, entry: &mut RomEntry) -> bool {
        if !self.mounted || enum_state.finished {
            return false;
        }

        // Local buffer to avoid borrow conflicts
        let mut dir_entry = [0u8; DIR_ENTRY_SIZE];

        loop {
            // Load sector if needed
            if !enum_state.sector_loaded {
                let sector_lba = self.cluster_to_sector(enum_state.cluster)
                    + enum_state.sector_in_cluster as u32;

                if self.sd.read_sector(sector_lba, &mut enum_state.sector_data).is_err() {
                    enum_state.finished = true;
                    return false;
                }
                enum_state.sector_loaded = true;
            }

            // Copy current entry to local buffer (avoids borrow conflict)
            let offset = enum_state.entry_in_sector * DIR_ENTRY_SIZE;
            dir_entry.copy_from_slice(&enum_state.sector_data[offset..offset + DIR_ENTRY_SIZE]);

            let first_byte = dir_entry[0];

            // Advance to next entry for next iteration
            enum_state.entry_in_sector += 1;
            if enum_state.entry_in_sector >= ENTRIES_PER_SECTOR {
                enum_state.entry_in_sector = 0;
                enum_state.sector_in_cluster += 1;
                enum_state.sector_loaded = false;

                if enum_state.sector_in_cluster >= self.sectors_per_cluster {
                    enum_state.sector_in_cluster = 0;
                    // Move to next cluster
                    match self.get_next_cluster(enum_state.cluster) {
                        Ok(next) if !Self::is_end_of_chain(next) => {
                            enum_state.cluster = next;
                        }
                        _ => {
                            enum_state.finished = true;
                        }
                    }
                }
            }

            // End of directory marker
            if first_byte == 0x00 {
                enum_state.finished = true;
                return false;
            }

            // Deleted entry - skip
            if first_byte == 0xE5 {
                enum_state.clear_lfn();
                continue;
            }

            let attr = dir_entry[11];

            // LFN entry - accumulate
            if (attr & attr::LONG_NAME_MASK) == attr::LONG_NAME {
                enum_state.process_lfn_entry(&dir_entry);
                continue;
            }

            // Skip volume label and directories
            if (attr & attr::VOLUME_ID) != 0 || (attr & attr::DIRECTORY) != 0 {
                enum_state.clear_lfn();
                continue;
            }

            // Regular file entry - check if it's a ROM
            if Self::is_rom_extension(&dir_entry) {
                // Validate LFN checksum if we have one
                if enum_state.lfn_valid {
                    let name_8_3: [u8; 11] = dir_entry[0..11].try_into().unwrap();
                    let checksum = DirEnumerator::calc_checksum(&name_8_3);
                    if checksum != enum_state.lfn_checksum {
                        enum_state.lfn_valid = false;
                    }
                }

                // Fill in entry
                if enum_state.lfn_valid {
                    enum_state.copy_lfn_to_entry(entry);
                } else {
                    DirEnumerator::copy_8_3_to_entry(&dir_entry, entry);
                }

                // Extract cluster and size
                let cluster_lo = u16::from_le_bytes([dir_entry[26], dir_entry[27]]);
                let cluster_hi = u16::from_le_bytes([dir_entry[20], dir_entry[21]]);
                entry.cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                entry.size = u32::from_le_bytes([
                    dir_entry[28],
                    dir_entry[29],
                    dir_entry[30],
                    dir_entry[31],
                ]);
                entry.is_gbc = Self::is_gbc_extension(&dir_entry);

                // Clear LFN state for next file
                enum_state.clear_lfn();

                return true;
            }

            // Not a ROM - clear LFN and continue
            enum_state.clear_lfn();
        }
    }

    /// Count total ROMs in directory
    pub fn count_roms(&mut self) -> usize {
        let mut enumerator = self.enumerate_roms();
        let mut entry = RomEntry::empty();
        let mut count = 0;

        while self.next_rom(&mut enumerator, &mut entry) {
            count += 1;
        }

        count
    }

    /// Read a file by its starting cluster
    pub fn read_file(
        &mut self,
        cluster: u32,
        size: u32,
        buffer: &mut [u8],
    ) -> Result<usize, &'static str> {
        if !self.mounted {
            return Err("Not mounted");
        }
        if cluster < 2 {
            return Err("Invalid cluster");
        }

        let to_read = (size as usize).min(buffer.len());
        let mut bytes_read = 0;
        let mut current_cluster = cluster;
        let mut sector_buf = [0u8; SECTOR_SIZE];

        while bytes_read < to_read && !Self::is_end_of_chain(current_cluster) {
            let cluster_lba = self.cluster_to_sector(current_cluster);

            for s in 0..self.sectors_per_cluster {
                if bytes_read >= to_read {
                    break;
                }

                self.sd.read_sector(cluster_lba + s as u32, &mut sector_buf)?;

                let copy_len = (to_read - bytes_read).min(SECTOR_SIZE);
                buffer[bytes_read..bytes_read + copy_len]
                    .copy_from_slice(&sector_buf[..copy_len]);
                bytes_read += copy_len;
            }

            current_cluster = self.get_next_cluster(current_cluster)?;
        }

        Ok(bytes_read)
    }
}

// ============================================================================
// ROM Selector FileSystem Trait Implementation
// ============================================================================

/// Adapter to use Fat32 with rom_selector
pub struct Fat32FileSystem<'a> {
    fs: &'a mut Fat32,
    enumerator: DirEnumerator,
}

impl<'a> Fat32FileSystem<'a> {
    pub fn new(fs: &'a mut Fat32) -> Self {
        let enumerator = fs.enumerate_roms();
        Self { fs, enumerator }
    }
}

impl<'a> crate::subsystems::rom_selector::FileSystem for Fat32FileSystem<'a> {
    fn reset_enumeration(&mut self) {
        self.enumerator = self.fs.enumerate_roms();
    }

    fn next_rom(&mut self, entry: &mut crate::subsystems::rom_selector::RomEntry) -> bool {
        let mut fat_entry = RomEntry::empty();

        if self.fs.next_rom(&mut self.enumerator, &mut fat_entry) {
            let copy_len = fat_entry.name_len.min(crate::subsystems::rom_selector::MAX_FILENAME_LEN);
            entry.name[..copy_len].copy_from_slice(&fat_entry.name[..copy_len]);
            entry.name_len = copy_len;
            entry.cluster = fat_entry.cluster;
            entry.size = fat_entry.size;
            entry.is_gbc = fat_entry.is_gbc;
            true
        } else {
            false
        }
    }
}
