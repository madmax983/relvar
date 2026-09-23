//! Lock abstraction for the storage layer.
//!
//! - Default (`std` feature): re-exports [`std::sync::RwLock`]. Zero behavior
//!   change, including poisoning semantics.
//! - `no_std` (`--no-default-features`): a thin wrapper over [`spin::RwLock`].
//!
//! A bare `pub use spin::RwLock` is not a drop-in replacement: spin's
//! `read()`/`write()` return the guard directly (spin locks never poison),
//! while `std`'s return `Result`. The wrapper below mirrors the `std` API —
//! `read`/`write` return `Result`, and the error type offers `into_inner`
//! like [`std::sync::PoisonError`] — so every call site compiles unchanged
//! under both configurations.

#[cfg(feature = "std")]
pub(crate) use std::sync::RwLock;

#[cfg(not(feature = "std"))]
pub(crate) use shim::RwLock;

#[cfg(not(feature = "std"))]
mod shim {
    /// Error type for the `no_std` lock shim, mirroring [`std::sync::PoisonError`].
    ///
    /// Spin locks never poison, so this is never constructed in practice. It
    /// exists so call sites written against the `std` API — `.read().unwrap()`,
    /// `.write().map_err(...)?`, and the `.unwrap_or_else(|e| e.into_inner())`
    /// poisoning-recovery pattern — compile unchanged.
    ///
    /// `Debug` is implemented manually (without a `T: Debug` bound) so
    /// `.unwrap()` works for any `T`, exactly like `std`'s `PoisonError`.
    pub(crate) enum LockError<T> {
        /// Uninhabited in practice: spin has no poisoning.
        #[allow(dead_code)]
        Poisoned(T),
    }

    impl<T> core::fmt::Debug for LockError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("LockError(..)")
        }
    }

    impl<T> LockError<T> {
        /// Recovers the guard, mirroring [`std::sync::PoisonError::into_inner`].
        /// Unreachable in practice (spin locks never poison).
        pub(crate) fn into_inner(self) -> T {
            let LockError::Poisoned(guard) = self;
            guard
        }
    }

    /// `no_std` reader-writer lock with the [`std::sync::RwLock`] API shape.
    ///
    /// Wraps [`spin::RwLock`]; `read`/`write` return `Result` so call sites
    /// written against `std` keep compiling.
    #[derive(Debug)]
    pub(crate) struct RwLock<T> {
        inner: spin::RwLock<T>,
    }

    impl<T> RwLock<T> {
        pub(crate) fn new(value: T) -> Self {
            Self {
                inner: spin::RwLock::new(value),
            }
        }

        pub(crate) fn read(
            &self,
        ) -> Result<spin::RwLockReadGuard<'_, T>, LockError<spin::RwLockReadGuard<'_, T>>> {
            Ok(self.inner.read())
        }

        pub(crate) fn write(
            &self,
        ) -> Result<spin::RwLockWriteGuard<'_, T>, LockError<spin::RwLockWriteGuard<'_, T>>>
        {
            Ok(self.inner.write())
        }
    }
}
