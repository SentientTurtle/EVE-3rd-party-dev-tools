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
#[cfg(feature = "enable_fsd")]   // TODO: Add macOS compatibility
pub mod fsd;

/// Module for ".static" data; Which are SQLite databases
pub mod static_sqlite {
    use std::collections::HashMap;
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use rusqlite::Connection;
    use serde::Deserialize;
    use crate::cache::{CacheError, SharedCache};

    #[derive(Debug)]
    pub enum StaticDataError {
        Cache(CacheError),
        Sqlite(rusqlite::Error),
        Json(serde_json::Error)
    }

    impl Display for StaticDataError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                StaticDataError::Cache(err) => Display::fmt(err, f),
                StaticDataError::Sqlite(err) => Display::fmt(err, f),
                StaticDataError::Json(err) => Display::fmt(err, f)
            }
        }
    }
    impl Error for StaticDataError {}
    impl From<CacheError> for StaticDataError { fn from(value: CacheError) -> Self { StaticDataError::Cache(value) } }
    impl From<rusqlite::Error> for StaticDataError { fn from(value: rusqlite::Error) -> Self { StaticDataError::Sqlite(value) } }
    impl From<serde_json::Error> for StaticDataError { fn from(value: serde_json::Error) -> Self { StaticDataError::Json(value) } }


    #[allow(non_snake_case)]
    #[derive(Deserialize)]
    pub struct SkinLicense {
        pub licenseTypeID: u32,
        pub skinID: u32,
        pub duration: i32,
        pub isSingleUse: Option<bool>
    }

    pub fn load_skin_licenses<C: SharedCache>(cache: &C) -> Result<HashMap<u32, SkinLicense>, StaticDataError> {
        let path = cache.path_of("res:/staticdata/skinlicenses.static")?;
        let connection = Connection::open(path)?;

        let mut skin_map = HashMap::<u32, SkinLicense>::new();

        let mut st = connection.prepare("SELECT value FROM cache")?;
        for value in st.query(())?.mapped(|r| r.get::<_, String>(0)) {
            let skin_license = serde_json::from_str::<SkinLicense>(&value?)?;
            skin_map.insert(skin_license.licenseTypeID, skin_license);
        }

        Ok(skin_map)
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    pub enum SkinDescription { // Mixed localizationString ID numbers & inline strings
        LocalizationID(u64),
        String(String),
    }

    #[allow(non_snake_case)]
    #[derive(Deserialize)]
    pub struct Skin {
        pub internalName: String,
        pub skinMaterialID: u32,
        pub visibleTranquility: bool,
        pub isStructureSkin: Option<bool>,
        pub skinDescription: Option<SkinDescription>,
        pub skinID: u32,
        pub allowCCPDevs: bool,
        pub visibleSerenity: bool,
        pub types: Vec<u32>
    }

    pub fn load_skins<C: SharedCache>(cache: &C) -> Result<HashMap<u32, Skin>, StaticDataError> {
        let path = cache.path_of("res:/staticdata/skins.static")?;
        let connection = Connection::open(path)?;

        let mut skin_map = HashMap::<u32, Skin>::new();

        let mut st = connection.prepare("SELECT value FROM cache")?;
        for value in st.query(())?.mapped(|r| r.get::<_, String>(0)) {
            let skin = serde_json::from_str::<Skin>(&value?)?;
            skin_map.insert(skin.skinID, skin);
        }

        Ok(skin_map)
    }
}