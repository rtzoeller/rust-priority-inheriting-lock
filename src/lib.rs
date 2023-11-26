//! This library provides a futex-based priority-inheriting lock implementation.
//!
//! It uses [@m-ou-se](https://github.com/m-ou-se/)'s [`linux-futex`](https://docs.rs/linux-futex/latest/linux_futex/) crate
//! to implement [@Amanieu](https://github.com/Amanieu/)'s [`lock_api`](https://docs.rs/lock_api/latest/lock_api/), providing
//! a priority-inheriting mutex on Linux.
//!
//! In general, you should consider using the lock implementations provided by `std` or `parking_lot`, unless your application
//! is intended to run on a real-time system where [priority inversions](https://en.wikipedia.org/wiki/Priority_inversion) must be avoided.

use std::sync::atomic::Ordering;

thread_local! {
    static TID: libc::pid_t = gettid();
}

/// A priority-inheriting lock implementation.
///
/// Consider using [`PriorityInheritingLock`] or [`SharedPriorityInheritingLock`] as higher level
/// abstractions around this type.
#[repr(transparent)]
pub struct RawPriorityInheritingLock<S>(linux_futex::PiFutex<S>);

impl<S> RawPriorityInheritingLock<S> {
    /// Create a new, unlocked `RawPriorityInheritingLock`.
    #[must_use]
    pub const fn new() -> Self {
        Self(linux_futex::PiFutex::new(0))
    }
}

impl<S> Default for RawPriorityInheritingLock<S> {
    /// Create a new, unlocked `RawPriorityInheritingLock`.
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<S: linux_futex::Scope> lock_api::RawMutex for RawPriorityInheritingLock<S> {
    // Following lock_api's design
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: RawPriorityInheritingLock<S> = RawPriorityInheritingLock::new();

    type GuardMarker = lock_api::GuardNoSend;

    /// Acquires this mutex, blocking the current thread until it is able to do so.
    /// If this call blocks and the thread holding the lock has a lower priority,
    /// the priority of the thread holding the lock will be temporarily boosted
    /// to match the calling thread's priority.
    fn lock(&self) {
        let tid = get_cached_tid();
        let fast_locked =
            self.0
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        if fast_locked.is_err() {
            while self.0.lock_pi().is_err() {}
        }
    }

    fn try_lock(&self) -> bool {
        let tid = get_cached_tid();
        let fast_locked =
            self.0
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        match fast_locked {
            Ok(_) => true,
            Err(_) => self.0.trylock_pi().is_ok(),
        }
    }

    unsafe fn unlock(&self) {
        let tid = get_cached_tid();
        let fast_unlocked =
            self.0
                .value
                .compare_exchange(tid as u32, 0, Ordering::SeqCst, Ordering::SeqCst);

        if fast_unlocked.is_err() {
            self.0.unlock_pi();
        }
    }
}

unsafe impl<S: linux_futex::Scope> lock_api::RawMutexTimed for RawPriorityInheritingLock<S> {
    type Duration = std::time::Duration;

    type Instant = std::time::Instant;

    fn try_lock_for(&self, timeout: Self::Duration) -> bool {
        self.try_lock_until(Self::Instant::now() + timeout)
    }

    fn try_lock_until(&self, timeout: Self::Instant) -> bool {
        let tid = get_cached_tid();
        let fast_locked =
            self.0
                .value
                .compare_exchange(0, tid as u32, Ordering::SeqCst, Ordering::SeqCst);

        if fast_locked.is_ok() {
            return true;
        }

        loop {
            match self.0.lock_pi_until(timeout) {
                Ok(_) => return true,
                Err(linux_futex::TimedLockError::TryAgain) => (),
                Err(linux_futex::TimedLockError::TimedOut) => return false,
            }
        }
    }
}

/// A priority-inheriting lock implementation, for use within a single process.
///
/// See also: [`PriorityInheritingLockGuard`], [`SharedPriorityInheritingLock`]
pub type PriorityInheritingLock<T> =
    lock_api::Mutex<RawPriorityInheritingLock<linux_futex::Private>, T>;

/// An RAII implementation of a "scoped lock" for [`PriorityInheritingLock`]. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` and `DerefMut` implementations.
pub type PriorityInheritingLockGuard<'a, T> =
    lock_api::MutexGuard<'a, RawPriorityInheritingLock<linux_futex::Private>, T>;

/// A priority-inheriting lock implementation, for use across processes.
///
/// See also: [`SharedPriorityInheritingLockGuard`], [`PriorityInheritingLock`]
pub type SharedPriorityInheritingLock<T> =
    lock_api::Mutex<RawPriorityInheritingLock<linux_futex::Shared>, T>;

/// An RAII implementation of a "scoped lock" for [`SharedPriorityInheritingLock`]. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` and `DerefMut` implementations.
pub type SharedPriorityInheritingLockGuard<'a, T> =
    lock_api::MutexGuard<'a, RawPriorityInheritingLock<linux_futex::Shared>, T>;

/// Safe wrapper around `gettid`.
///
/// `gettid` is always successful on Linux.
#[must_use]
pub fn gettid() -> libc::pid_t {
    unsafe { libc::gettid() }
}

#[inline]
fn get_cached_tid() -> libc::pid_t {
    let mut tid = 0;
    TID.with(|it| {
        tid = *it;
    });

    tid
}
