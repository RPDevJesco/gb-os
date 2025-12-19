//! FAT32 Filesystem Driver - Clean Version
//!
//! Provides read-only FAT32 filesystem support for loading ROM files.

// =============================================================================
// Constants
// =============================================================================

const SECTOR_SIZE: usize = 512;
const FIRST_DATA_CLUSTER: u32 = 2;

// =============================================================================
// FAT32 Filesystem
// =============================================================================

/// FAT32 filesystem state
pub struct Fat32 {
    device_index: usize,
    mounted: bool,
    partition_start: u32,
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    fat_start_sector: u32,
    sectors_per_fat: u32,
    data_start_sector: u32,
    root_cluster: u32,
}

impl Fat32 {
    pub const fn new() -> Self {
        Self {
            device_index: 0,
            mounted: false,
            partition_start: 0,
            bytes_per_sector: 512,
            sectors_per_cluster: 1,
            fat_start_sector: 0,
            sectors_per_fat: 0,
            data_start_sector: 0,
            root_cluster: 0,
        }
    }

    /// Mount FAT32 filesystem
    pub fn mount(&mut self, device_index: usize) -> Result<(), &'static str> {
        self.device_index = device_index;

        let device = crate::storage::ata::get_device(device_index)
            .ok_or("Device not found")?;

        // Read sector 0
        let mut sector = [0u8; SECTOR_SIZE];
        crate::storage::ata::read_sectors(device, 0, 1, &mut sector)
            .map_err(|_| "Failed to read sector 0")?;

        // Check signature
        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid signature");
        }

        // Check if MBR or VBR
        let potential_bps = u16::from_le_bytes([sector[11], sector[12]]);
        let is_valid_bps = potential_bps == 512 || potential_bps == 1024 ||
            potential_bps == 2048 || potential_bps == 4096;
        let part1_type = sector[446 + 4];
        let has_fat32_partition = part1_type == 0x0B || part1_type == 0x0C;

        // If MBR with FAT32 partition, read VBR
        if has_fat32_partition && !is_valid_bps {
            self.partition_start = u32::from_le_bytes([
                sector[446 + 8], sector[446 + 9],
                sector[446 + 10], sector[446 + 11],
            ]);

            crate::storage::ata::read_sectors(device, self.partition_start as u64, 1, &mut sector)
                .map_err(|_| "Failed to read VBR")?;

            if sector[510] != 0x55 || sector[511] != 0xAA {
                return Err("Invalid VBR signature");
            }
        } else {
            self.partition_start = 0;
        }

        // Parse BPB
        let bytes_per_sector = u16::from_le_bytes([sector[11], sector[12]]);
        let sectors_per_cluster = sector[13];
        let reserved_sectors = u16::from_le_bytes([sector[14], sector[15]]);
        let num_fats = sector[16];
        let sectors_per_fat_32 = u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]);
        let root_cluster = u32::from_le_bytes([sector[44], sector[45], sector[46], sector[47]]);

        // Validate
        if bytes_per_sector < 512 || bytes_per_sector > 4096 { return Err("Bad BPS"); }
        if sectors_per_cluster == 0 { return Err("Bad SPC"); }
        if num_fats == 0 { return Err("Bad FATs"); }
        if root_cluster < 2 { return Err("Bad root"); }

        // Store parameters
        self.bytes_per_sector = bytes_per_sector as u32;
        self.sectors_per_cluster = sectors_per_cluster as u32;
        self.fat_start_sector = self.partition_start + reserved_sectors as u32;
        self.sectors_per_fat = sectors_per_fat_32;
        self.root_cluster = root_cluster;
        self.data_start_sector = self.fat_start_sector + (num_fats as u32 * sectors_per_fat_32);

        self.mounted = true;
        Ok(())
    }

    pub fn is_mounted(&self) -> bool { self.mounted }

    fn cluster_to_sector(&self, cluster: u32) -> u64 {
        let offset = cluster - FIRST_DATA_CLUSTER;
        (self.data_start_sector + offset * self.sectors_per_cluster) as u64
    }

    /// Read a sector
    fn read_sector(&self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        let device = crate::storage::ata::get_device(self.device_index)
            .ok_or("No device")?;
        crate::storage::ata::read_sectors(device, lba, 1, buf)
            .map_err(|_| "Read failed")?;
        Ok(())
    }

    /// Find a .gb or .gbc file in root directory
    /// Returns (first_cluster, file_size) if found
    pub fn find_rom(&self, index: usize) -> Option<(u32, u32)> {
        if !self.mounted { return None; }

        let mut sector = [0u8; SECTOR_SIZE];
        let mut rom_index = 0usize;
        let mut current_cluster = self.root_cluster;

        // Follow cluster chain for root directory
        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster);

            // Read each sector in this cluster
            for sector_offset in 0..self.sectors_per_cluster {
                if self.read_sector(cluster_lba + sector_offset as u64, &mut sector).is_err() {
                    return None;
                }

                for i in 0..16 {  // 16 entries per sector
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 { return None; }  // End of directory
                    if first_byte == 0xE5 { continue; }  // Deleted

                    let attr = sector[offset + 11];
                    if attr == 0x0F { continue; }  // LFN entry
                    if attr == 0x08 { continue; }  // Volume label
                    if (attr & 0x10) != 0 { continue; }  // Directory

                    // Check extension (offset 8-10) - case insensitive
                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    // Match .GB or .GBC
                    let is_gb = ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C');

                    if is_gb {
                        if rom_index == index {
                            let cluster_lo = u16::from_le_bytes([sector[offset + 26], sector[offset + 27]]);
                            let cluster_hi = u16::from_le_bytes([sector[offset + 20], sector[offset + 21]]);
                            let cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                            let size = u32::from_le_bytes([
                                sector[offset + 28], sector[offset + 29],
                                sector[offset + 30], sector[offset + 31],
                            ]);
                            return Some((cluster, size));
                        }
                        rom_index += 1;
                    }
                }
            }

            // Get next cluster in chain
            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        None
    }

    /// Get filename of ROM at index (8.3 format)
    pub fn get_rom_name(&self, index: usize, name_buf: &mut [u8; 12]) -> bool {
        if !self.mounted { return false; }

        let mut sector = [0u8; SECTOR_SIZE];
        let mut rom_index = 0usize;
        let mut current_cluster = self.root_cluster;

        // Follow cluster chain for root directory
        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster);

            for sector_offset in 0..self.sectors_per_cluster {
                if self.read_sector(cluster_lba + sector_offset as u64, &mut sector).is_err() {
                    return false;
                }

                for i in 0..16 {
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 { return false; }
                    if first_byte == 0xE5 { continue; }

                    let attr = sector[offset + 11];
                    if attr == 0x0F { continue; }
                    if attr == 0x08 { continue; }
                    if (attr & 0x10) != 0 { continue; }

                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    let is_gb = ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C');

                    if is_gb {
                        if rom_index == index {
                            // Copy name (8 chars) + dot + ext (3 chars)
                            let mut pos = 0;
                            for j in 0..8 {
                                let c = sector[offset + j];
                                if c != b' ' {
                                    name_buf[pos] = c;
                                    pos += 1;
                                }
                            }
                            name_buf[pos] = b'.';
                            pos += 1;
                            for j in 0..3 {
                                let c = sector[offset + 8 + j];
                                if c != b' ' {
                                    name_buf[pos] = c;
                                    pos += 1;
                                }
                            }
                            while pos < 12 {
                                name_buf[pos] = 0;
                                pos += 1;
                            }
                            return true;
                        }
                        rom_index += 1;
                    }
                }
            }

            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        false
    }

    /// Count ROM files in root directory
    pub fn count_roms(&self) -> usize {
        if !self.mounted { return 0; }

        let mut sector = [0u8; SECTOR_SIZE];
        let mut count = 0;
        let mut current_cluster = self.root_cluster;

        // Debug row 197: show root_cluster value
        unsafe {
            let vga = 0xA0000 as *mut u8;
            core::ptr::write_volatile(vga.add(197 * 320), (self.root_cluster & 0xFF) as u8);
            core::ptr::write_volatile(vga.add(197 * 320 + 1), ((self.root_cluster >> 8) & 0xFF) as u8);
            core::ptr::write_volatile(vga.add(197 * 320 + 2), ((self.root_cluster >> 16) & 0xFF) as u8);
        }

        // Follow cluster chain for root directory
        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster);

            // Debug row 197: show cluster LBA
            unsafe {
                let vga = 0xA0000 as *mut u8;
                core::ptr::write_volatile(vga.add(197 * 320 + 10), (cluster_lba & 0xFF) as u8);
                core::ptr::write_volatile(vga.add(197 * 320 + 11), ((cluster_lba >> 8) & 0xFF) as u8);
                core::ptr::write_volatile(vga.add(197 * 320 + 12), ((cluster_lba >> 16) & 0xFF) as u8);
            }

            for sector_offset in 0..self.sectors_per_cluster {
                let read_result = self.read_sector(cluster_lba + sector_offset as u64, &mut sector);

                // Debug row 198: show read result
                unsafe {
                    let vga = 0xA0000 as *mut u8;
                    let color = if read_result.is_ok() { 0x0A } else { 0x04 };
                    core::ptr::write_volatile(vga.add(198 * 320 + sector_offset as usize * 3), color);
                }

                if read_result.is_err() {
                    return count;
                }

                // Debug row 199: show first 16 bytes of directory
                unsafe {
                    let vga = 0xA0000 as *mut u8;
                    for i in 0..16 {
                        core::ptr::write_volatile(vga.add(199 * 320 + i), sector[i]);
                    }
                }

                for i in 0..16 {
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 {
                        // Debug: yellow pixel = hit end marker
                        unsafe {
                            let vga = 0xA0000 as *mut u8;
                            core::ptr::write_volatile(vga.add(198 * 320 + 20), 0x0E);
                        }
                        return count;
                    }
                    if first_byte == 0xE5 { continue; }

                    let attr = sector[offset + 11];
                    if attr == 0x0F { continue; }
                    if attr == 0x08 { continue; }
                    if (attr & 0x10) != 0 { continue; }

                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    let is_gb = ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C');

                    if is_gb {
                        count += 1;
                        // Debug: green pixel for each ROM found
                        unsafe {
                            let vga = 0xA0000 as *mut u8;
                            core::ptr::write_volatile(vga.add(198 * 320 + 30 + count), 0x0A);
                        }
                    }
                }
            }

            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        count
    }

    /// Read file data into buffer
    /// Returns bytes read
    pub fn read_file(&self, cluster: u32, size: u32, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if !self.mounted { return Err("Not mounted"); }
        if cluster < 2 { return Err("Invalid cluster"); }

        let to_read = (size as usize).min(buffer.len());
        let mut bytes_read = 0usize;
        let mut current_cluster = cluster;

        let mut sector_buf = [0u8; SECTOR_SIZE];

        while bytes_read < to_read && current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster);

            // Read each sector in cluster
            for s in 0..self.sectors_per_cluster {
                if bytes_read >= to_read { break; }

                self.read_sector(cluster_lba + s as u64, &mut sector_buf)?;

                let copy_len = (to_read - bytes_read).min(SECTOR_SIZE);
                buffer[bytes_read..bytes_read + copy_len].copy_from_slice(&sector_buf[..copy_len]);
                bytes_read += copy_len;
            }

            // Get next cluster from FAT
            current_cluster = self.get_next_cluster(current_cluster)?;
        }

        Ok(bytes_read)
    }

    fn get_next_cluster(&self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let entry_offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut sector = [0u8; SECTOR_SIZE];
        self.read_sector(fat_sector as u64, &mut sector)?;

        let next = u32::from_le_bytes([
            sector[entry_offset],
            sector[entry_offset + 1],
            sector[entry_offset + 2],
            sector[entry_offset + 3],
        ]) & 0x0FFFFFFF;

        Ok(next)
    }
}

// Global instance
static mut FAT32_FS: Fat32 = Fat32::new();

pub fn get_fs() -> &'static mut Fat32 {
    unsafe { &mut FAT32_FS }
}

pub fn mount(device_index: usize) -> Result<(), &'static str> {
    get_fs().mount(device_index)
}

pub fn is_mounted() -> bool {
    get_fs().is_mounted()
}
