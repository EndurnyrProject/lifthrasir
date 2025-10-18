use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for async request/response correlation
/// Uses atomic counter to generate monotonically increasing IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

static COUNTER: AtomicU64 = AtomicU64::new(1);

impl RequestId {
    /// Generate a new unique RequestId
    pub fn new() -> Self {
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RequestId({})", self.0)
    }
}
