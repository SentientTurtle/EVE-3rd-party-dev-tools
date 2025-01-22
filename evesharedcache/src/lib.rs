#![feature(iter_next_chunk)]
#![feature(exit_status_error)]

/// General SharedCache module
///
/// Provides the [`cache::SharedCache`] trait for reading the EVE Online game file cache, with two implementations:
/// * [`cache::CacheReader`] provides READ-ONLY access to a locally-installed copy of the game
/// * [`cache::CacheDownloader`] provides access to the game file CDN, creating a local on-disk cache
pub mod cache;


/// Module for "FSD" data. Unpacking requires running a binary python library, and so is unavailable on certain operating systems.
///
/// Currently only supports windows
#[cfg(target_os = "windows")]   // TODO: Add macOS compatibility
pub mod fsd;