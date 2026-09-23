//! # Relvar Storage Core — the storage hardware-abstraction boundary
//!
//! `relvar-storage-core` is the `no_std`+`alloc` foundation of Relvar's v0.7
//! storage HAL (hardware abstraction layer). It owns everything about the
//! **on-disk format** that must be identical on every target — server, laptop,
//! or microcontroller — while knowing nothing about files, sockets, or any
//! other OS service.
//!
//! ## The HAL boundary
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ relvar-storage        │ host side: File-backed devices, │
//! │                       │ page cache, WAL manager, heap   │
//! │                       │ files, recovery (needs std)     │
//! ├───────────────────────┼─────────────────────────────────┤
//! │ relvar-storage-core   │ ★ THIS CRATE ★                  │
//! │ (no_std + alloc)      │ on-disk formats + HAL traits:   │
//! │                       │                                 │
//! │  page    │ 4 KiB page layout: 8-byte LE length header,  │
//! │          │ zero-padded to PAGE_SIZE                     │
//! │  slotted │ slotted-page layouts (v1 legacy + v2 MVCC    │
//! │          │ length-prefixed), exact current framing      │
//! │  wal     │ LSN / TransactionId types, WalRecord enum    │
//! │          │ (postcard repr frozen), v2 CRC-32 frames and │
//! │          │ legacy frame decoding                       │
//! │  crc     │ IEEE CRC-32 (pure core, const table)         │
//! │  device  │ BlockDevice trait + MemBlockDevice: the     │
//! │          │ single seam between formats and hardware    │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Portability contract
//!
//! * **Byte-compatibility is mandatory.** The structs, the postcard version,
//!   and therefore the bytes on disk are identical with and without the
//!   `std` feature. A WAL or heap file written on a server must decode on
//!   a `thumbv8m` target and vice versa.
//! * **No I/O here.** The only way bytes enter or leave this crate is
//!   `BlockDevice` (page-sized reads/writes/flush) or
//!   through pure encode/decode functions over caller-supplied buffers.
//!   A host implements `BlockDevice` over a file; an embedded target
//!   implements it over flash.
//! * **No panics on corrupt input.** Every decode path returns a `Result`;
//!   `.unwrap()`/`.expect()` never appear in non-test code.
//!
//! ## TTM terminology
//!
//! Following the house rules: *relation* (not table), *tuple* (not row),
//! *attribute* (not column), *relvar* (not table variable), *heading*
//! (not schema). Physical identifiers such as `TupleId` are
//! storage-internal and never cross into the logical layer
//! (TTM Proscription 6).

// `#![no_std]` + `alloc` when the `std` feature is off (mirrors relvar-core).
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

// Modules land one per commit; each owns one on-disk format plus its tests.

/// 4 KiB page layout: 8-byte little-endian length header, zero-padded to
/// [`PAGE_SIZE`](page::PAGE_SIZE). The lowest-level framing; everything else
/// is built on pages.
pub mod page;

/// IEEE CRC-32 (pure `core`, `const`-computed table). Integrity primitive
/// for the WAL v2 frame format (torn-write detection).
pub mod crc;

/// LSNs, transaction IDs, WAL records, and WAL framing (v2 CRC-32 frames
/// plus legacy pre-v0.7 frame decoding). The postcard representation of
/// [`WalRecord`](wal::WalRecord) is frozen for byte-compatibility.
pub mod wal;

/// Slotted-page payload layouts (v1 legacy + v2 MVCC) with their exact
/// on-disk framing. [`TupleId`](slotted::TupleId) is storage-internal per
/// TTM Proscription 6.
pub mod slotted;
