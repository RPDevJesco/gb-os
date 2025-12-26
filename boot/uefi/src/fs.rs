//! File System Operations
//! 
//! Load files from UEFI file system (FAT32 typically).

use core::ffi::c_void;
use crate::uefi::*;
use crate::memory;
use crate::println;

/// File handle wrapper
pub struct File {
    protocol: *mut EFI_FILE_PROTOCOL,
}

impl File {
    /// Read entire file into a newly allocated buffer
    pub fn read_all(&self) -> Result<(*mut u8, usize), EFI_STATUS> {
        // Get file size
        let mut info_size: UINTN = 0;
        let mut status = unsafe {
            ((*self.protocol).get_info)(
                self.protocol,
                &EFI_FILE_INFO_GUID,
                &mut info_size,
                core::ptr::null_mut(),
            )
        };
        
        if status != EFI_BUFFER_TOO_SMALL {
            return Err(status);
        }
        
        // Allocate buffer for file info
        let info_buffer = memory::allocate_pool(info_size, EFI_MEMORY_TYPE::LoaderData)?;
        
        status = unsafe {
            ((*self.protocol).get_info)(
                self.protocol,
                &EFI_FILE_INFO_GUID,
                &mut info_size,
                info_buffer,
            )
        };
        
        if status != EFI_SUCCESS {
            let _ = memory::free_pool(info_buffer);
            return Err(status);
        }
        
        let file_size = unsafe { (*(info_buffer as *const EFI_FILE_INFO)).file_size as usize };
        let _ = memory::free_pool(info_buffer);
        
        // Allocate buffer for file content
        let buffer = memory::allocate_pool(file_size, EFI_MEMORY_TYPE::LoaderData)? as *mut u8;
        let mut read_size = file_size;
        
        status = unsafe {
            ((*self.protocol).read)(self.protocol, &mut read_size, buffer as *mut c_void)
        };
        
        if status != EFI_SUCCESS {
            let _ = memory::free_pool(buffer as *mut c_void);
            return Err(status);
        }
        
        Ok((buffer, read_size))
    }
    
    /// Read into provided buffer
    #[allow(dead_code)]
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, EFI_STATUS> {
        let mut size = buffer.len();
        
        let status = unsafe {
            ((*self.protocol).read)(self.protocol, &mut size, buffer.as_mut_ptr() as *mut c_void)
        };
        
        if status == EFI_SUCCESS {
            Ok(size)
        } else {
            Err(status)
        }
    }
    
    /// Get current position
    #[allow(dead_code)]
    pub fn position(&self) -> Result<u64, EFI_STATUS> {
        let mut pos: u64 = 0;
        
        let status = unsafe { ((*self.protocol).get_position)(self.protocol, &mut pos) };
        
        if status == EFI_SUCCESS {
            Ok(pos)
        } else {
            Err(status)
        }
    }
    
    /// Set position
    #[allow(dead_code)]
    pub fn seek(&self, position: u64) -> Result<(), EFI_STATUS> {
        let status = unsafe { ((*self.protocol).set_position)(self.protocol, position) };
        
        if status == EFI_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe {
            ((*self.protocol).close)(self.protocol);
        }
    }
}

/// File system handle
pub struct FileSystem {
    root: *mut EFI_FILE_PROTOCOL,
}

impl FileSystem {
    /// Open the file system from a loaded image
    /// 
    /// # Safety
    /// Caller must ensure boot_services and image_handle are valid
    pub unsafe fn from_loaded_image(
        boot_services: *mut EFI_BOOT_SERVICES,
        image_handle: EFI_HANDLE,
    ) -> Result<Self, EFI_STATUS> {
        unsafe {
            // Get loaded image protocol
            let mut loaded_image: *mut c_void = core::ptr::null_mut();
            
            let status = ((*boot_services).handle_protocol)(
                image_handle,
                &EFI_LOADED_IMAGE_PROTOCOL_GUID,
                &mut loaded_image,
            );
            
            if status != EFI_SUCCESS {
                return Err(status);
            }
            
            let device_handle = (*(loaded_image as *mut EFI_LOADED_IMAGE_PROTOCOL)).device_handle;
            
            // Get file system protocol
            let mut fs_protocol: *mut c_void = core::ptr::null_mut();
            
            let status = ((*boot_services).handle_protocol)(
                device_handle,
                &EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID,
                &mut fs_protocol,
            );
            
            if status != EFI_SUCCESS {
                return Err(status);
            }
            
            // Open root directory
            let mut root: *mut EFI_FILE_PROTOCOL = core::ptr::null_mut();
            
            let status = ((*(fs_protocol as *mut EFI_SIMPLE_FILE_SYSTEM_PROTOCOL)).open_volume)(
                fs_protocol as *mut EFI_SIMPLE_FILE_SYSTEM_PROTOCOL,
                &mut root,
            );
            
            if status != EFI_SUCCESS {
                return Err(status);
            }
            
            Ok(Self { root })
        }
    }
    
    /// Open a file by path
    pub fn open(&self, path: &str, mode: u64) -> Result<File, EFI_STATUS> {
        // Convert path to UTF-16
        let mut path_buf = [0u16; 256];
        for (i, c) in path.chars().enumerate() {
            if i >= 255 {
                break;
            }
            // Convert forward slashes to backslashes (UEFI uses backslashes)
            path_buf[i] = if c == '/' { '\\' as u16 } else { c as u16 };
        }
        
        let mut file: *mut EFI_FILE_PROTOCOL = core::ptr::null_mut();
        
        let status = unsafe {
            ((*self.root).open)(
                self.root,
                &mut file,
                path_buf.as_ptr(),
                mode,
                0, // attributes (for create)
            )
        };
        
        if status == EFI_SUCCESS {
            Ok(File { protocol: file })
        } else {
            Err(status)
        }
    }
    
    /// Open a file for reading
    pub fn open_read(&self, path: &str) -> Result<File, EFI_STATUS> {
        self.open(path, EFI_FILE_MODE_READ)
    }
    
    /// Load a file completely into memory
    pub fn load_file(&self, path: &str) -> Result<(*mut u8, usize), EFI_STATUS> {
        let file = self.open_read(path)?;
        file.read_all()
    }
    
    /// Check if a file exists
    pub fn exists(&self, path: &str) -> bool {
        self.open_read(path).is_ok()
    }
}

impl Drop for FileSystem {
    fn drop(&mut self) {
        unsafe {
            ((*self.root).close)(self.root);
        }
    }
}

/// Load a kernel file and return its entry point
pub fn load_kernel(
    fs: &FileSystem,
    path: &str,
) -> Result<(u64, *mut u8, usize), EFI_STATUS> {
    println!("Loading kernel: {}", path);
    
    let (buffer, size) = fs.load_file(path)?;
    
    println!("  Loaded {} bytes at {:p}", size, buffer);
    
    // For a real kernel, you'd parse the ELF/PE header here
    // and extract the entry point. For now, assume entry is at start.
    let entry_point = buffer as u64;
    
    Ok((entry_point, buffer, size))
}
