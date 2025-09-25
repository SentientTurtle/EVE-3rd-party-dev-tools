#![allow(non_snake_case, non_camel_case_types)] // Extensive use of serialized types, whose names match the output fields

use crate::icons::TypeInfo;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::{fs, io};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

#[derive(Serialize, Deserialize)]
#[serde(tag = "_key")]
enum SdeVersion {
    sde { buildNumber: u32, releaseDate: String }
}

fn parse_version(document: &str) -> Result<u32, io::Error> {
    for line in document.lines() {
        return match serde_json::from_str::<SdeVersion>(line)? {
            SdeVersion::sde { buildNumber, .. } => Ok(buildNumber)
        }
    }
    Err(io::Error::other("missing _sde entry in latest.jsonl!"))
}

pub fn get_sde_version() -> Result<u32, io::Error> {
    let doc = reqwest::blocking::get("https://developers.eveonline.com/static-data/tranquility/latest.jsonl")
        .map_err(io::Error::other)?
        .text().map_err(io::Error::other)?;

    parse_version(&*doc)
}

pub fn download_sde<W: Write>(dest: &mut W) -> Result<(), io::Error> {
    reqwest::blocking::get("https://developers.eveonline.com/static-data/eve-online-static-data-latest-jsonl.zip")
        .map_err(io::Error::other)?
        .copy_to(dest)
        .map(|_| ())
        .map_err(io::Error::other)
}

#[allow(unused_parens)] // Occasionally over-eager, so disabled for now
pub fn update_sde(silent_mode: bool) -> Result<ZipArchive<File>, io::Error> {
    let mut download = true;
    let new_version = get_sde_version()?;
    if fs::exists("./cache/sde.zip")? {
        let mut version_doc = String::new();
        ZipArchive::new(File::open("./cache/sde.zip")?)?.by_name("_sde.jsonl")?.read_to_string(&mut version_doc)?;
        let current_version = parse_version(&*version_doc)?;
        download = (current_version != new_version);
    }
    if download {
        if !silent_mode { println!("Downloading new SDE..."); }
        fs::create_dir_all("./cache")?;
        download_sde(&mut File::create("./cache/sde.zip")?)?;
    }
    if !silent_mode { println!("SDE up to date!"); }

    Ok(ZipArchive::new(File::open("./cache/sde.zip")?)?)
}

#[derive(Deserialize)]
pub struct Keyed<T> {
    _key: u32,
    #[serde(flatten)]
    content: T
}

pub fn read_types(sde: &mut ZipArchive<File>, silent_mode: bool) -> Result<HashMap<u32, TypeInfo>, io::Error> {
    // Parsing the SDEs YAML properly is rather slow and fragile as the SDE is not entirely spec-compliant, so we just directly extract the fields we need.
    if !silent_mode { println!("\tLoading types..."); }
    let mut types = HashMap::<u32, TypeInfo>::new();
    for line in BufReader::new(sde.by_name("types.jsonl")?).lines() {
        let type_info = serde_json::from_str::<Keyed<TypeInfo>>(&*line?)?;
        types.insert(type_info._key, type_info.content);
    }
    types.retain(|_, info| info.graphic_id.is_some() || info.icon_id.is_some() || (1950..=1955).contains(&info.group_id) || info.group_id == 4040);
    Ok(types)
}

pub fn read_group_categories(sde: &mut ZipArchive<File>, silent_mode: bool) -> Result<HashMap<u32, u32>, io::Error> {
    if !silent_mode { println!("\tLoading groups..."); }
    let mut group_categories = HashMap::<u32, u32>::new();
    for line in BufReader::new(sde.by_name("groups.jsonl")?).lines() {
        #[derive(Deserialize)]
        struct Group { categoryID: u32 }
        let group_info = serde_json::from_str::<Keyed<Group>>(&*line?)?;

        group_categories.insert(group_info._key, group_info.content.categoryID);
    }
    Ok(group_categories)
}

pub fn read_icons(sde: &mut ZipArchive<File>, silent_mode: bool) -> Result<HashMap<u32, String>, io::Error> {
    if !silent_mode { println!("\tLoading icon info..."); }
    let mut icon_files = HashMap::<u32, String>::new();
    for line in BufReader::new(sde.by_name("icons.jsonl")?).lines() {
        #[derive(Deserialize)]
        struct Icon { iconFile: String }
        let icon_info = serde_json::from_str::<Keyed<Icon>>(&*line?)?;

        icon_files.insert(icon_info._key, icon_info.content.iconFile);
    }
    Ok(icon_files)
}

pub fn read_graphics(sde: &mut ZipArchive<File>, silent_mode: bool) -> Result<HashMap<u32, String>, io::Error> {
    if !silent_mode { println!("\tLoading graphic info..."); }
    let mut graphic_folders = HashMap::<u32, String>::new();
    for line in BufReader::new(sde.by_name("graphics.jsonl")?).lines() {
        #[derive(Deserialize)]
        struct Graphic { iconFolder: Option<String> }
        let graphic_info = serde_json::from_str::<Keyed<Graphic>>(&*line?)?;

        if let Some(folder) = graphic_info.content.iconFolder {
            graphic_folders.insert(graphic_info._key, folder);
        }
    }
    Ok(graphic_folders)
}

pub fn read_skin_materials(sde: &mut ZipArchive<File>, silent_mode: bool) -> Result<HashMap<u32, u32>, io::Error> {
    if !silent_mode { println!("\tLoading skin info..."); }
    let mut license_skins = HashMap::<u32, u32>::new();
    for line in BufReader::new(sde.by_name("skinLicenses.jsonl")?).lines() {
        #[derive(Deserialize)]
        struct SkinLicense { skinID: u32 }
        let license_info = serde_json::from_str::<Keyed<SkinLicense>>(&*line?)?;
        license_skins.insert(license_info._key, license_info.content.skinID);
    }

    let mut skin_materials = HashMap::<u32, u32>::new();
    for line in BufReader::new(sde.by_name("skinMaterials.jsonl")?).lines() {
        #[derive(Deserialize)]
        struct SkinMaterial { skinMaterialID: u32 }
        let material_info = serde_json::from_str::<Keyed<SkinMaterial>>(&*line?)?;
        skin_materials.insert(material_info._key, material_info.content.skinMaterialID);
    }

    let mut license_materials = HashMap::new();
    for (license_id, skin_id) in license_skins {
        // Some unused licenses exist in the data, but their associated skins do not exist
        if let Some(material) = skin_materials.get(&skin_id) {
            license_materials.insert(license_id, *material);
        }
    }

    Ok(license_materials)
}