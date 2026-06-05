/// General SharedCache module
///
/// Provides the [`cache::SharedCache`] trait for reading the EVE Online game file cache, with two implementations:
/// * [`cache::CacheReader`] provides READ-ONLY access to a locally-installed copy of the game
/// * [`cache::CacheDownloader`] provides access to the game file CDN, creating a local on-disk cache
pub mod cache;

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const CRATE_REPO: &'static str = env!("CARGO_PKG_REPOSITORY");

#[cfg(test)]
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