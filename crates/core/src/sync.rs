//! Locks that outlive a panic.
//!
//! A `std::sync::Mutex` is "poisoned" when a thread panics while holding it, and every later
//! `.lock().unwrap()` then panics too: one bug in one turn would break speaking, listening or
//! settings until Pixie restarts. `lock_ok` takes the lock anyway. What's inside may be
//! half-updated, which for Pixie's state (settings, caches, the current turn) is far better
//! than never being able to use it again.

use std::sync::{Mutex, MutexGuard};

pub trait LockOk<T: ?Sized> {
    /// The lock, even after a panic poisoned it.
    fn lock_ok(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockOk<T> for Mutex<T> {
    fn lock_ok(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_panic_while_locked_does_not_break_the_lock() {
        let m = Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("a bug while holding the lock");
        })
        .join();
        assert!(m.is_poisoned());
        *m.lock_ok() += 1;
        assert_eq!(*m.lock_ok(), 2);
    }
}
