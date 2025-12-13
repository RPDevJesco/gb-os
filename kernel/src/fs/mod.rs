//! Filesystem Layer
//!
//! Rustacean OS filesystem support with Plan 9-style "everything is a file" philosophy.
//! Supports FAT16 for ROM loading from partitions and exFAT for USB compatibility.

pub mod exfat;
pub mod fat16;

/// Maximum path length
pub const MAX_PATH: usize = 256;

/// Maximum filename length
pub const MAX_FILENAME: usize = 255;

/// File types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file
    Regular,
    /// Directory
    Directory,
    /// Symbolic link
    Symlink,
    /// Block device
    BlockDevice,
    /// Character device
    CharDevice,
    /// Named pipe (FIFO)
    Pipe,
    /// Unix socket
    Socket,
}

/// File open flags
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub create: bool,
    pub truncate: bool,
    pub exclusive: bool,
}

impl OpenFlags {
    /// Read-only access
    pub const fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            append: false,
            create: false,
            truncate: false,
            exclusive: false,
        }
    }
    
    /// Write-only access
    pub const fn write_only() -> Self {
        Self {
            read: false,
            write: true,
            append: false,
            create: false,
            truncate: false,
            exclusive: false,
        }
    }
    
    /// Read-write access
    pub const fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            append: false,
            create: false,
            truncate: false,
            exclusive: false,
        }
    }
    
    /// Create file if it doesn't exist
    pub const fn with_create(mut self) -> Self {
        self.create = true;
        self
    }
    
    /// Truncate file on open
    pub const fn with_truncate(mut self) -> Self {
        self.truncate = true;
        self
    }
}

/// File permissions (Unix-style)
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    /// Owner permissions
    pub owner: PermissionBits,
    /// Group permissions
    pub group: PermissionBits,
    /// Other permissions
    pub other: PermissionBits,
}

/// Permission bits
#[derive(Debug, Clone, Copy)]
pub struct PermissionBits {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    /// Default file permissions (644)
    pub const fn default_file() -> Self {
        Self {
            owner: PermissionBits { read: true, write: true, execute: false },
            group: PermissionBits { read: true, write: false, execute: false },
            other: PermissionBits { read: true, write: false, execute: false },
        }
    }
    
    /// Default directory permissions (755)
    pub const fn default_dir() -> Self {
        Self {
            owner: PermissionBits { read: true, write: true, execute: true },
            group: PermissionBits { read: true, write: false, execute: true },
            other: PermissionBits { read: true, write: false, execute: true },
        }
    }
}

/// File metadata
#[derive(Debug, Clone)]
pub struct Metadata {
    /// File type
    pub file_type: FileType,
    /// File size in bytes
    pub size: u64,
    /// Permissions
    pub permissions: Permissions,
    /// Creation time (Unix timestamp)
    pub created: u64,
    /// Last modification time
    pub modified: u64,
    /// Last access time
    pub accessed: u64,
}

/// Directory entry
#[derive(Debug)]
pub struct DirEntry {
    /// Entry name
    pub name: [u8; MAX_FILENAME],
    pub name_len: usize,
    /// File type
    pub file_type: FileType,
    /// Inode number (or cluster for exFAT)
    pub inode: u64,
}

impl DirEntry {
    /// Get name as string slice
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("???")
    }
}

/// Filesystem error
#[derive(Debug, Clone, Copy)]
pub enum FsError {
    /// File not found
    NotFound,
    /// Permission denied
    PermissionDenied,
    /// File already exists
    AlreadyExists,
    /// Not a directory
    NotDirectory,
    /// Is a directory
    IsDirectory,
    /// Invalid path
    InvalidPath,
    /// Filesystem full
    NoSpace,
    /// Too many open files
    TooManyOpenFiles,
    /// I/O error
    IoError,
    /// Filesystem not mounted
    NotMounted,
    /// Invalid filesystem
    InvalidFs,
    /// Read-only filesystem
    ReadOnly,
}

/// Filesystem result type
pub type FsResult<T> = Result<T, FsError>;

/// Virtual filesystem trait
///
/// All filesystems implement this trait for unified access.
pub trait Filesystem {
    /// Get filesystem name
    fn name(&self) -> &'static str;
    
    /// Mount the filesystem
    fn mount(&mut self) -> FsResult<()>;
    
    /// Unmount the filesystem
    fn unmount(&mut self) -> FsResult<()>;
    
    /// Open a file
    fn open(&mut self, path: &str, flags: OpenFlags) -> FsResult<u64>;
    
    /// Close a file
    fn close(&mut self, handle: u64) -> FsResult<()>;
    
    /// Read from file
    fn read(&mut self, handle: u64, buf: &mut [u8]) -> FsResult<usize>;
    
    /// Write to file
    fn write(&mut self, handle: u64, buf: &[u8]) -> FsResult<usize>;
    
    /// Seek in file
    fn seek(&mut self, handle: u64, offset: i64, whence: SeekFrom) -> FsResult<u64>;
    
    /// Get file metadata
    fn stat(&self, path: &str) -> FsResult<Metadata>;
    
    /// Read directory entries
    fn readdir(&mut self, path: &str) -> FsResult<ReadDir>;
    
    /// Create a directory
    fn mkdir(&mut self, path: &str) -> FsResult<()>;
    
    /// Remove a file
    fn remove(&mut self, path: &str) -> FsResult<()>;
    
    /// Remove a directory
    fn rmdir(&mut self, path: &str) -> FsResult<()>;
    
    /// Rename/move a file
    fn rename(&mut self, from: &str, to: &str) -> FsResult<()>;
}

/// Seek origin
#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    /// Seek from start of file
    Start,
    /// Seek from current position
    Current,
    /// Seek from end of file
    End,
}

/// Directory iterator
pub struct ReadDir {
    /// Entries (fixed size for no_std)
    entries: [Option<DirEntry>; 64],
    /// Number of entries
    count: usize,
    /// Current index
    index: usize,
}

impl ReadDir {
    /// Create empty directory listing
    pub const fn empty() -> Self {
        const NONE: Option<DirEntry> = None;
        Self {
            entries: [NONE; 64],
            count: 0,
            index: 0,
        }
    }
    
    /// Add an entry
    pub fn add(&mut self, entry: DirEntry) -> bool {
        if self.count < 64 {
            self.entries[self.count] = Some(entry);
            self.count += 1;
            true
        } else {
            false
        }
    }
}

impl Iterator for ReadDir {
    type Item = DirEntry;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let entry = self.entries[self.index].take();
            self.index += 1;
            entry
        } else {
            None
        }
    }
}
