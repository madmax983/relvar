//! `no_std`-compatible collection aliases.
//!
//! When the `std` cargo feature is disabled this crate builds as `#![no_std]`,
//! where `std::collections::{HashMap, HashSet}` do not exist. These aliases
//! keep a single code path on every target: `hashbrown` tables keyed with the
//! `no_std`-compatible `foldhash` hasher.

use foldhash::fast::RandomState;

/// A hash map that works with and without `std`.
///
/// Backed by `hashbrown` with a `no_std`-compatible hasher; API-compatible
/// with `std::collections::HashMap` for everything this crate uses.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, RandomState>;

/// A hash set that works with and without `std`.
///
/// Backed by `hashbrown` with a `no_std`-compatible hasher; API-compatible
/// with `std::collections::HashSet` for everything this crate uses.
pub type HashSet<T> = hashbrown::HashSet<T, RandomState>;

/// Build a fresh hasher for ad-hoc hashing.
///
/// Replaces `std::collections::hash_map::DefaultHasher` in `no_std` builds.
/// The hasher carries a fresh random state, matching the behavior the crate
/// previously got from the standard hasher.
pub fn new_hasher() -> impl core::hash::Hasher {
    use core::hash::BuildHasher;
    RandomState::default().build_hasher()
}
