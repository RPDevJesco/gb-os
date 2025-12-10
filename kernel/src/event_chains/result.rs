//! Event Result - no_std implementation
//!
//! Result type for event execution in Rustacean OS.

/// Maximum error message length
const MAX_ERROR_LEN: usize = 128;

/// Error message storage (no heap allocation)
#[derive(Clone, Copy)]
pub struct ErrorMessage {
    data: [u8; MAX_ERROR_LEN],
    len: usize,
}

impl ErrorMessage {
    /// Create from a static string
    pub const fn from_static(s: &'static str) -> Self {
        let bytes = s.as_bytes();
        let mut data = [0u8; MAX_ERROR_LEN];
        let len = if bytes.len() > MAX_ERROR_LEN { MAX_ERROR_LEN } else { bytes.len() };
        
        // Manual copy since we're in const context
        let mut i = 0;
        while i < len {
            data[i] = bytes[i];
            i += 1;
        }
        
        Self { data, len }
    }
    
    /// Create from a string slice
    pub fn from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        let mut data = [0u8; MAX_ERROR_LEN];
        let len = bytes.len().min(MAX_ERROR_LEN);
        data[..len].copy_from_slice(&bytes[..len]);
        Self { data, len }
    }
    
    /// Get the message as a string slice
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.len]).unwrap_or("invalid utf8")
    }
}

impl core::fmt::Debug for ErrorMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl core::fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Result of event execution
#[derive(Debug, Clone, Copy)]
pub enum EventResult<T> {
    /// Event executed successfully
    Success(T),
    /// Event failed (business logic error)
    Failure(ErrorMessage),
    /// Middleware infrastructure failed
    MiddlewareFailure(ErrorMessage),
}

impl<T> EventResult<T> {
    /// Create a success result
    pub fn success(value: T) -> Self {
        Self::Success(value)
    }
    
    /// Create a failure result from static string
    pub fn failure(msg: &'static str) -> Self {
        Self::Failure(ErrorMessage::from_static(msg))
    }
    
    /// Create a failure result from string slice
    pub fn failure_str(msg: &str) -> Self {
        Self::Failure(ErrorMessage::from_str(msg))
    }
    
    /// Create a middleware failure from static string
    pub fn middleware_failure(msg: &'static str) -> Self {
        Self::MiddlewareFailure(ErrorMessage::from_static(msg))
    }
    
    /// Check if this is a success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }
    
    /// Check if this is any kind of failure
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure(_) | Self::MiddlewareFailure(_))
    }
    
    /// Check if this is specifically an event failure
    pub fn is_event_failure(&self) -> bool {
        matches!(self, Self::Failure(_))
    }
    
    /// Check if this is a middleware failure
    pub fn is_middleware_failure(&self) -> bool {
        matches!(self, Self::MiddlewareFailure(_))
    }
    
    /// Get the error message if this is a failure
    pub fn error_message(&self) -> Option<&ErrorMessage> {
        match self {
            Self::Failure(msg) | Self::MiddlewareFailure(msg) => Some(msg),
            Self::Success(_) => None,
        }
    }
    
    /// Unwrap the success value, panicking on failure
    pub fn unwrap(self) -> T {
        match self {
            Self::Success(v) => v,
            Self::Failure(msg) => panic!("Event failure: {}", msg),
            Self::MiddlewareFailure(msg) => panic!("Middleware failure: {}", msg),
        }
    }
    
    /// Get the success value or a default
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Self::Success(v) => v,
            _ => default,
        }
    }
    
    /// Map the success value
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> EventResult<U> {
        match self {
            Self::Success(v) => EventResult::Success(f(v)),
            Self::Failure(msg) => EventResult::Failure(msg),
            Self::MiddlewareFailure(msg) => EventResult::MiddlewareFailure(msg),
        }
    }
}

impl<T: Default> EventResult<T> {
    /// Get the success value or default
    pub fn unwrap_or_default(self) -> T {
        match self {
            Self::Success(v) => v,
            _ => T::default(),
        }
    }
}

/// Chain execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainStatus {
    /// All events completed successfully
    Completed,
    /// Chain completed but some events failed (Lenient/BestEffort mode)
    CompletedWithWarnings,
    /// Chain failed and stopped early
    Failed,
}

impl core::fmt::Display for ChainStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Completed => write!(f, "COMPLETED"),
            Self::CompletedWithWarnings => write!(f, "COMPLETED_WITH_WARNINGS"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// Maximum number of failures to track
const MAX_FAILURES: usize = 16;

/// Event failure record
#[derive(Debug, Clone, Copy)]
pub struct EventFailure {
    /// Name of the failed event
    pub event_name: &'static str,
    /// Error message
    pub error: ErrorMessage,
    /// Whether this was a middleware failure
    pub is_middleware_failure: bool,
}

/// Result of chain execution
#[derive(Debug)]
pub struct ChainResult {
    /// Overall success status
    pub success: bool,
    /// Chain status
    pub status: ChainStatus,
    /// Recorded failures
    failures: [Option<EventFailure>; MAX_FAILURES],
    /// Number of failures
    failure_count: usize,
}

impl ChainResult {
    /// Create a successful chain result
    pub fn success() -> Self {
        Self {
            success: true,
            status: ChainStatus::Completed,
            failures: [None; MAX_FAILURES],
            failure_count: 0,
        }
    }
    
    /// Create a partial success (some failures in lenient mode)
    pub fn partial_success() -> Self {
        Self {
            success: true,
            status: ChainStatus::CompletedWithWarnings,
            failures: [None; MAX_FAILURES],
            failure_count: 0,
        }
    }
    
    /// Create a failed chain result
    pub fn failed() -> Self {
        Self {
            success: false,
            status: ChainStatus::Failed,
            failures: [None; MAX_FAILURES],
            failure_count: 0,
        }
    }
    
    /// Add a failure record
    pub fn add_failure(&mut self, failure: EventFailure) {
        if self.failure_count < MAX_FAILURES {
            self.failures[self.failure_count] = Some(failure);
            self.failure_count += 1;
        }
    }
    
    /// Get the number of failures
    pub fn failure_count(&self) -> usize {
        self.failure_count
    }
    
    /// Iterate over failures
    pub fn failures(&self) -> impl Iterator<Item = &EventFailure> {
        self.failures[..self.failure_count].iter().filter_map(|f| f.as_ref())
    }
}
