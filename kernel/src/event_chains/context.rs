//! Event Context - no_std implementation
//!
//! A fixed-capacity key-value store for passing data through event chains.
//! Uses static arrays instead of HashMap.

/// Maximum number of context entries
const MAX_ENTRIES: usize = 32;

/// Maximum key length
const MAX_KEY_LEN: usize = 32;

/// Context entry
struct ContextEntry {
    key: [u8; MAX_KEY_LEN],
    key_len: usize,
    value: ContextValue,
    occupied: bool,
}

impl ContextEntry {
    const fn empty() -> Self {
        Self {
            key: [0; MAX_KEY_LEN],
            key_len: 0,
            value: ContextValue::None,
            occupied: false,
        }
    }
}

/// Typed context values
/// 
/// Since we can't use `dyn Any` without allocation, we use an enum
/// of common types used in the kernel.
#[derive(Clone, Copy)]
pub enum ContextValue {
    None,
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Usize(usize),
    Isize(isize),
    Ptr(*const u8),
    MutPtr(*mut u8),
}

/// Event context - carries data through the event chain
pub struct EventContext {
    entries: [ContextEntry; MAX_ENTRIES],
    count: usize,
}

impl EventContext {
    /// Create a new empty context
    pub const fn new() -> Self {
        const EMPTY: ContextEntry = ContextEntry::empty();
        Self {
            entries: [EMPTY; MAX_ENTRIES],
            count: 0,
        }
    }
    
    /// Set a boolean value
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set_value(key, ContextValue::Bool(value));
    }
    
    /// Get a boolean value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get_value(key)? {
            ContextValue::Bool(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Set a u32 value
    pub fn set_u32(&mut self, key: &str, value: u32) {
        self.set_value(key, ContextValue::U32(value));
    }
    
    /// Get a u32 value
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        match self.get_value(key)? {
            ContextValue::U32(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Set a u64 value
    pub fn set_u64(&mut self, key: &str, value: u64) {
        self.set_value(key, ContextValue::U64(value));
    }
    
    /// Get a u64 value
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        match self.get_value(key)? {
            ContextValue::U64(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Set a usize value
    pub fn set_usize(&mut self, key: &str, value: usize) {
        self.set_value(key, ContextValue::Usize(value));
    }
    
    /// Get a usize value
    pub fn get_usize(&self, key: &str) -> Option<usize> {
        match self.get_value(key)? {
            ContextValue::Usize(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Set a raw pointer value
    pub fn set_ptr(&mut self, key: &str, value: *const u8) {
        self.set_value(key, ContextValue::Ptr(value));
    }
    
    /// Get a raw pointer value
    pub fn get_ptr(&self, key: &str) -> Option<*const u8> {
        match self.get_value(key)? {
            ContextValue::Ptr(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Set a mutable raw pointer value
    pub fn set_mut_ptr(&mut self, key: &str, value: *mut u8) {
        self.set_value(key, ContextValue::MutPtr(value));
    }
    
    /// Get a mutable raw pointer value
    pub fn get_mut_ptr(&self, key: &str) -> Option<*mut u8> {
        match self.get_value(key)? {
            ContextValue::MutPtr(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Check if a key exists
    pub fn has(&self, key: &str) -> bool {
        self.find_key(key).is_some()
    }
    
    /// Remove a key
    pub fn remove(&mut self, key: &str) {
        if let Some(idx) = self.find_key(key) {
            self.entries[idx].occupied = false;
            self.count -= 1;
        }
    }
    
    /// Clear all entries
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.occupied = false;
        }
        self.count = 0;
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    // Internal helpers
    
    fn set_value(&mut self, key: &str, value: ContextValue) {
        // Try to find existing key
        if let Some(idx) = self.find_key(key) {
            self.entries[idx].value = value;
            return;
        }
        
        // Find empty slot
        if self.count >= MAX_ENTRIES {
            return; // Full, silently fail (could panic in debug)
        }
        
        for entry in self.entries.iter_mut() {
            if !entry.occupied {
                let key_bytes = key.as_bytes();
                let copy_len = key_bytes.len().min(MAX_KEY_LEN);
                entry.key[..copy_len].copy_from_slice(&key_bytes[..copy_len]);
                entry.key_len = copy_len;
                entry.value = value;
                entry.occupied = true;
                self.count += 1;
                return;
            }
        }
    }
    
    fn get_value(&self, key: &str) -> Option<&ContextValue> {
        let idx = self.find_key(key)?;
        Some(&self.entries[idx].value)
    }
    
    fn find_key(&self, key: &str) -> Option<usize> {
        let key_bytes = key.as_bytes();
        
        for (idx, entry) in self.entries.iter().enumerate() {
            if entry.occupied && 
               entry.key_len == key_bytes.len() &&
               &entry.key[..entry.key_len] == key_bytes 
            {
                return Some(idx);
            }
        }
        
        None
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}
