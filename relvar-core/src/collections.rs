//! `no_std`-compatible collection aliases.
//!
//! When the `std` cargo feature is disabled this crate builds as `#![no_std]`,
//! where `std::collections::{HashMap, HashSet}` do not exist. These aliases
//! keep a single code path on every target: `hashbrown` tables keyed with its
//! default `foldhash`-based hasher, which needs no operating system.

/// A hash map that works with and without `std`.
///
/// Backed by `hashbrown` with its default `no_std`-compatible hasher;
/// API-compatible with `std::collections::HashMap` for everything this crate
/// uses.
pub type HashMap<K, V> = hashbrown::HashMap<K, V>;

/// A hash set that works with and without `std`.
///
/// Backed by `hashbrown` with its default `no_std`-compatible hasher;
/// API-compatible with `std::collections::HashSet` for everything this crate
/// uses.
pub type HashSet<T> = hashbrown::HashSet<T>;

/// Build a fresh hasher for ad-hoc hashing.
///
/// Replaces `std::collections::hash_map::DefaultHasher` in `no_std` builds.
/// The hasher carries a fresh random state, matching the behavior the crate
/// previously got from the standard hasher.
pub fn new_hasher() -> impl core::hash::Hasher {
    use core::hash::BuildHasher;
    hashbrown::DefaultHashBuilder::default().build_hasher()
}
