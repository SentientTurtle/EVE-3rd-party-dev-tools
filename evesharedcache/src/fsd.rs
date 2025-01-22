#![allow(non_snake_case)]   // Serialized types

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io;
use std::path::Path;
use std::process::Command;
use serde::Deserialize;
use crate::cache::{CacheError, SharedCache};

#[derive(Debug)]
pub enum FSDError {
    IO(io::Error),
    Cache(CacheError),
    FormatChange,
    PythonOops
}

impl Display for FSDError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FSDError::Cache(err) => Display::fmt(err, f),
            FSDError::IO(err) => Display::fmt(err, f),
            FSDError::PythonOops => write!(f, "A python error occurred, no error information is available, Try STDOUT"),
            FSDError::FormatChange => write!(f, "FSD format changed")
        }
    }
}

impl From<CacheError> for FSDError {
    fn from(value: CacheError) -> Self {
        FSDError::Cache(value)
    }
}

impl From<io::Error> for FSDError {
    fn from(value: io::Error) -> Self {
        FSDError::IO(value)
    }
}

impl Error for FSDError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FSDError::PythonOops => None,
            FSDError::Cache(err) => Some(err),
            FSDError::IO(err) => Some(err),
            FSDError::FormatChange => None
        }
    }
}

const FSD_TO_JSON_SCRIPT: &'static str = include_str!("fsd.py");

/// Unpacks an FSD file into a json file
///
/// Requires python 2.7 to be available on the current system, involves loading binary python libraries and is not available on certain operating systems
///
/// # Arguments
///
/// * `cache`: SharedCache to load from
/// * `python2`: Command/Path to python 2.7
/// * `fsd_dir`: (temp) directory to unpack into
/// * `fsdbinary_resource`: Cache resource of the binary to load
/// * `loader_resource`: Cache resource of the loader to use (generally "\[fsdbinary\]Loader"
/// * `json_name`: path for output file
///
/// returns: Result<(), FSDError>
pub fn unpack_fsd<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P, fsdbinary_resource: &str, loader_resource: &str, json_outfile: &str) -> Result<(), FSDError> {
    let loader_filename = loader_resource.split('/').last().unwrap();
    let loader_name = loader_filename.split('.').next().unwrap();

    let loader_path = fsd_dir.as_ref().join(loader_filename);
    let binary_path = std::path::absolute(cache.path_of(fsdbinary_resource)?)?;

    std::fs::copy(cache.path_of(loader_resource)?, &loader_path)?;

    let status = Command::new(python2)
        .current_dir(fsd_dir)
        .arg("-c")
        .arg(FSD_TO_JSON_SCRIPT)
        .arg(loader_name)
        .arg(binary_path)
        .arg(json_outfile)
        .status()?;

    std::fs::remove_file(&loader_path)?;

    if status.success() {
        Ok(())
    } else {
        Err(FSDError::PythonOops)
    }
}

// -- Types

/// Convenience wrapper around [`unpack_fsd`] for 'types.fsdbinary', writes to "types.json" in `fsd_dir`
///
/// # Arguments
///
/// * `cache`: SharedCache to load from
/// * `python2`: Command/Path to python 2.7
/// * `fsd_dir`: (temp) directory to unpack into
///
/// returns: Result<(), FSDError>
pub fn unpack_types<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<(), FSDError> {
    unpack_fsd(cache, python2, fsd_dir, "res:/staticdata/types.fsdbinary", "app:/bin64/typesLoader.pyd", "types.json")
}

#[derive(Deserialize, Debug, Clone)]
pub struct EVEType {
    pub typeID: u32,
    pub radius: f64,
    pub capacity: f64,
    pub raceID: Option<u32>,
    pub typeNameID: u32,
    pub basePrice: f64,
    pub volume: f64,
    pub mass: f64,
    pub published: u8,              // integer-boolean; 0=false,1=true,
    pub portionSize: u32,
    pub groupID: u32,
    pub descriptionID: Option<u32>,
    pub iconID: Option<u32>,
    pub marketGroupID: Option<u32>,
    pub graphicID: Option<u32>,
    pub isDynamicType: Option<u8>,  // integer-boolean; 0=false,1=true
    pub metaGroupID: Option<u32>,
    pub metaLevel: Option<u32>,
    pub variationParentTypeID: Option<u32>,
    pub techLevel: Option<u32>,
    pub wreckTypeID: Option<u32>,
    pub quoteID: Option<u32>,
    pub quoteAuthorID: Option<u32>,
    pub designerIDs: Option<Vec<u32>>,
    pub factionID: Option<u32>,
    pub isisGroupID: Option<u32>,
    pub soundID: Option<u32>,
    pub certificateTemplate: Option<u32>,
}

/// See [`unpack_types`], loads generated data using serde. Still requires a directory to write into, and does not delete files afterwards
pub fn read_types<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<HashMap<u32, EVEType>, FSDError> {
    unpack_fsd(cache, python2, fsd_dir.as_ref(), "res:/staticdata/types.fsdbinary", "app:/bin64/typesLoader.pyd", "types.json")?;
    serde_json::from_reader(File::open(fsd_dir.as_ref().join("types.json"))?).map_err(|_| FSDError::FormatChange)
}

// -- Groups
/// Convenience wrapper around [`unpack_fsd`] for 'groups.fsdbinary', writes to "groups.json" in `fsd_dir`
///
/// # Arguments
///
/// * `cache`: SharedCache to load from
/// * `python2`: Command/Path to python 2.7
/// * `fsd_dir`: (temp) directory to unpack into
///
/// returns: Result<(), FSDError>
pub fn unpack_groups<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<(), FSDError> {
    unpack_fsd(cache, python2, fsd_dir, "res:/staticdata/groups.fsdbinary", "app:/bin64/groupsLoader.pyd", "groups.json")
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct EVEGroup {
    pub groupID: u32,
    pub anchorable: u8,             // integer-boolean; 0=false,1=true,
    pub fittableNonSingleton: u8,   // integer-boolean; 0=false,1=true,
    pub groupNameID: u32,
    pub anchored: u8,               // integer-boolean; 0=false,1=true,
    pub published: u8,              // integer-boolean; 0=false,1=true,
    pub useBasePrice: u8,           // integer-boolean; 0=false,1=true,
    pub categoryID: u32,
    pub iconID: Option<u32>,
}

/// See [`unpack_groups`], loads generated data using serde. Still requires a directory to write into, and does not delete files afterwards
pub fn read_groups<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<HashMap<u32, EVEGroup>, FSDError> {
    unpack_fsd(cache, python2, fsd_dir.as_ref(), "res:/staticdata/groups.fsdbinary", "app:/bin64/groupsLoader.pyd", "groups.json")?;
    serde_json::from_reader(File::open(fsd_dir.as_ref().join("groups.json"))?).map_err(|_| FSDError::FormatChange)
}

// -- Icons
/// Convenience wrapper around [`unpack_fsd`] for 'iconids.fsdbinary', writes to "icons.json" in `fsd_dir`
///
/// # Arguments
///
/// * `cache`: SharedCache to load from
/// * `python2`: Command/Path to python 2.7
/// * `fsd_dir`: (temp) directory to unpack into
///
/// returns: Result<(), FSDError>
pub fn unpack_icons<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<(), FSDError> {
    unpack_fsd(cache, python2, fsd_dir, "res:/staticdata/iconids.fsdbinary", "app:/bin64/iconIDsLoader.pyd", "icons.json")
}

#[derive(Deserialize, Debug, Clone)]
pub struct EVEIcon {
    pub iconFile: String,
    pub iconType: Option<String>,
    pub obsolete: Option<u8>    // integer-boolean; 0=false,1=true
}
/// See [`unpack_icons`], loads generated data using serde. Still requires a directory to write into, and does not delete files afterwards
pub fn read_icons<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<HashMap<u32, EVEIcon>, FSDError> {
    unpack_fsd(cache, python2, fsd_dir.as_ref(), "res:/staticdata/iconids.fsdbinary", "app:/bin64/iconIDsLoader.pyd", "icons.json")?;
    serde_json::from_reader(File::open(fsd_dir.as_ref().join("icons.json"))?).map_err(|_| FSDError::FormatChange)
}

// -- Graphics
/// Convenience wrapper around [`unpack_fsd`] for 'graphicids.fsdbinary', writes to "graphics.json" in `fsd_dir`
///
/// # Arguments
///
/// * `cache`: SharedCache to load from
/// * `python2`: Command/Path to python 2.7
/// * `fsd_dir`: (temp) directory to unpack into
///
/// returns: Result<(), FSDError>
pub fn unpack_graphics<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<(), FSDError> {
    unpack_fsd(cache, python2, fsd_dir, "res:/staticdata/graphicids.fsdbinary", "app:/bin64/graphicIDsLoader.pyd", "graphics.json")
}

#[derive(Deserialize, Debug, Clone)]
pub struct EVEGraphicIconInfo {
    pub folder: String
}
#[derive(Deserialize, Debug, Clone)]
pub struct EVEGraphic {
    pub explosionBucketID: Option<u32>,
    pub iconInfo: Option<EVEGraphicIconInfo>,
    pub sofRaceName: Option<String>,
    pub sofFactionName: Option<String>,
    pub sofHullName: Option<String>,
    pub graphicFile: Option<String>,
    pub animationStateObjects: Option<HashMap<String, String>>,
    pub sofLayout: Option<Vec<String>>,
    pub controllerVariableOverrides: Option<HashMap<String, f64>>,
    pub graphicLocationID: Option<u32>,
    pub sofMaterialSetID: Option<u32>,
    pub ammoColor: Option<HashMap<String, f64>>,
    pub emissiveColor: Option<Vec<f64>>,
    pub albedoColor: Option<Vec<f64>>,
}

/// See [`unpack_graphics`], loads generated data using serde. Still requires a directory to write into, and does not delete files afterwards
pub fn read_graphics<C: SharedCache, P: AsRef<Path>>(cache: &C, python2: &str, fsd_dir: P) -> Result<HashMap<u32, EVEGraphic>, FSDError> {
    unpack_fsd(cache, python2, fsd_dir.as_ref(), "res:/staticdata/graphicids.fsdbinary", "app:/bin64/graphicIDsLoader.pyd", "graphics.json")?;
    serde_json::from_reader(File::open(fsd_dir.as_ref().join("graphics.json"))?).map_err(|_| FSDError::FormatChange)
}