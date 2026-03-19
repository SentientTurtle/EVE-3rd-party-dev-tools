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
#[cfg(feature = "enable_fsd")]
pub mod fsd;


pub mod test {
    use std::error::Error;
    use crate::cache;

    #[test]
    fn test() -> Result<(), Box<dyn Error>> {
        let downloader = cache::CacheDownloader::initialize("./cache", false, "")?;

        let (valid, invalid) = downloader.validate()?;

        println!("valid:{} invalid:{}", valid, invalid);

        Ok(())
    }
}