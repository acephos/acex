//! Shared-store helpers — recover from mutex poison instead of panicking.

use std::sync::{Mutex, MutexGuard};

use acex_model::Store;

/// Lock `Store`, recovering from poison (prior panic on another thread).
#[inline]
pub fn lock_store(store: &Mutex<Store>) -> MutexGuard<'_, Store> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
