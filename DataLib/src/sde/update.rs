#![allow(non_snake_case, non_camel_case_types)] // Use of serialized types, whose names match the output fields

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::{fs, io};
use zip::ZipArchive;

pub const VERSION_URL: &'static str = "https://developers.eveonline.com/static-data/tranquility/latest.jsonl";
pub const SDE_URL: &'static str = "https://developers.eveonline.com/static-data/eve-online-static-data-latest-jsonl.zip";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "_key")]
pub enum SdeVersion {
    sde { buildNumber: u32, releaseDate: String }
}

impl SdeVersion {
    pub fn try_zip<P: AsRef<Path>>(path: P) -> Result<SdeVersion, io::Error> {
        if fs::exists(&path)? {
            #[allow(unused_qualifications)]
            Self::from_sde(path)
        } else {
            Ok(SdeVersion::sde { buildNumber: 0, releaseDate: "".to_string() })
        }
    }

    pub fn from_sde<P: AsRef<Path>>(path: P) -> Result<SdeVersion, io::Error> {
        let mut archive = ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
        serde_json::from_reader(archive.by_name("_sde.jsonl").map_err(io::Error::other)?).map_err(io::Error::other)
    }

    pub fn from_file<R: Read>(read: R) -> Result<SdeVersion, io::Error> {
        serde_json::from_reader(read).map_err(io::Error::other)
    }

    pub fn download_latest() -> Result<SdeVersion, io::Error> {
        reqwest::blocking::get(VERSION_URL).map_err(io::Error::other)?
            .json::<SdeVersion>().map_err(io::Error::other)
    }
}

pub fn download_latest_sde<P: AsRef<Path>>(file: P) -> Result<SdeVersion, io::Error> {
    reqwest::blocking::get(SDE_URL).map_err(io::Error::other)?
        .copy_to(&mut File::create(&file)?).map(|_| ()).map_err(io::Error::other)?;

    SdeVersion::try_zip(file)
}

pub fn update_sde<P: AsRef<Path>>(file: P) -> Result<SdeVersion, io::Error> {
    let current @ SdeVersion::sde { buildNumber: current_version, .. } = SdeVersion::try_zip(&file)?;
    let SdeVersion::sde { buildNumber: latest, .. } = SdeVersion::download_latest()?;
    if current_version < latest {
        download_latest_sde(file)
    } else {
        Ok(current)
    }
}
