use crate::icons::TypeInfo;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::str::FromStr;
use std::{fs, io};
use yaml_rust2::{yaml, Yaml, YamlLoader};
use zip::ZipArchive;

pub fn get_fsd_checksum() -> Result<String, io::Error> {
    reqwest::blocking::get("https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/fsd.zip.checksum")
        .map_err(io::Error::other)?
        .text().map_err(io::Error::other)
}

pub fn download_fsd<W: Write>(dest: &mut W) -> Result<(), io::Error> {
    reqwest::blocking::get("https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/fsd.zip")
        .map_err(io::Error::other)?
        .copy_to(dest)
        .map(|_| ())
        .map_err(io::Error::other)
}

#[allow(unused_parens)] // Occasionally over-eager, so disabled for now
pub fn update_sde() -> Result<ZipArchive<File>, io::Error> {
    let mut download = true;
    let new_version = get_fsd_checksum()?;
    if fs::exists("./cache/fsd.zip")? && fs::exists("./cache/checksum.txt")? {
        let current_version = fs::read_to_string("./cache/checksum.txt")?;
        download = (current_version != new_version);
    }
    if download {
        println!("Downloading new SDE...");
        fs::create_dir_all("./cache")?;
        fs::write("./cache/checksum.txt", new_version)?;  // Store checksum in another file; The checksum is over the (unzipped) contents of the archive, not the archive itself, and so is hard/slow to calculate
        download_fsd(&mut File::create("./cache/fsd.zip")?)?;
    }
    println!("SDE up to date!");

    Ok(ZipArchive::new(File::open("./cache/fsd.zip")?)?)
}

pub fn read_types(sde: &mut ZipArchive<File>) -> Result<HashMap<u32, TypeInfo>, io::Error> {
    // Parsing the SDEs YAML properly is rather slow and fragile as the SDE is not entirely spec-compliant, so we just directly extract the fields we need.
    println!("\tLoading types...");
    let mut types = HashMap::<u32, TypeInfo>::new();
    {
        let mut current_type: Option<&mut TypeInfo> = None;
        for line in BufReader::new(sde.by_name("types.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                let type_id = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
                current_type = Some(types.entry(type_id).or_insert_with(TypeInfo::default));
            } else {
                if let Some(ref mut current_type) = current_type {
                    let line = line.trim();
                    if let Some(group_id) = line.strip_prefix("groupID: ") {
                        current_type.group_id = f64::from_str(group_id).map_err(io::Error::other)? as u32; // Some IDs are formatted as float
                    } else if let Some(icon_id) = line.strip_prefix("iconID: ") {
                        current_type.icon_id = Some(f64::from_str(icon_id).map_err(io::Error::other)? as u32);
                    } else if let Some(graphic_id) = line.strip_prefix("graphicID: ") {
                        current_type.graphic_id = Some(f64::from_str(graphic_id).map_err(io::Error::other)? as u32);
                    } else if let Some(graphic_id) = line.strip_prefix("metaGroupID: ") {
                        current_type.meta_group_id = Some(f64::from_str(graphic_id).map_err(io::Error::other)? as u32);
                    }
                }
            }
        }
        types.retain(|_, info| info.graphic_id.is_some() || info.icon_id.is_some() || (1950..=1955).contains(&info.group_id) || info.group_id == 4040);
    }
    Ok(types)
}

pub fn read_group_categories(sde: &mut ZipArchive<File>) -> Result<HashMap<u32, u32>, io::Error> {
    println!("\tLoading groups...");
    let mut group_categories = HashMap::<u32, u32>::new();
    {
        let mut current_group = 0u32;
        for line in BufReader::new(sde.by_name("groups.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                current_group = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
            } else {
                let line = line.trim();
                if let Some(category_id) = line.strip_prefix("categoryID: ") {
                    group_categories.insert(current_group, u32::from_str(category_id).map_err(io::Error::other)?);
                }
            }
        }
    }
    Ok(group_categories)
}

pub fn read_icons(sde: &mut ZipArchive<File>) -> Result<HashMap<u32, String>, io::Error> {
    println!("\tLoading icon info...");
    let mut icon_files = HashMap::<u32, String>::new();
    {
        let mut file = sde.by_name("iconIDs.yaml")?;
        let mut buffer = String::with_capacity(file.size() as usize);
        file.read_to_string(&mut buffer)?;

        let [yaml]: [Yaml; 1] = YamlLoader::load_from_str(&buffer).map_err(io::Error::other)?.try_into().map_err(|_| io::Error::other("Malformed SDE YAML"))?;

        let icon_file_key = Yaml::String("iconFile".to_string());

        for (key, value) in yaml.into_hash().into_iter().flat_map(yaml::Hash::into_iter) {
            if let (Yaml::Integer(icon_id), Yaml::Hash(value)) = (key, value) {
                if let Some(icon_file) = value.get(&icon_file_key).and_then(Yaml::as_str) {
                    icon_files.insert(icon_id as u32, icon_file.to_string());
                }
            }
        }
    }
    Ok(icon_files)
}

pub fn read_graphics(sde: &mut ZipArchive<File>) -> Result<HashMap<u32, String>, io::Error> {
    println!("\tLoading graphic info...");
    let mut graphic_folders = HashMap::<u32, String>::new();
    {
        let mut current_graphic = 0u32;
        for line in BufReader::new(sde.by_name("graphicIDs.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                current_graphic = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
            } else {
                let line = line.trim();
                if let Some(icon_file) = line.strip_prefix("folder: ") {
                    graphic_folders.insert(current_graphic, icon_file.trim().to_string());
                }
            }
        }
    }
    Ok(graphic_folders)
}

pub fn read_skin_materials(sde: &mut ZipArchive<File>) -> Result<HashMap<u32, u32>, io::Error> {
    println!("\tLoading skin info...");
    let mut license_skins = HashMap::<u32, u32>::new();
    {
        let mut current_license = 0u32;
        for line in BufReader::new(sde.by_name("skinLicenses.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                current_license = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
            } else {
                let line = line.trim();
                if let Some(icon_file) = line.strip_prefix("skinID: ") {
                    license_skins.insert(current_license, icon_file.trim().parse().map_err(io::Error::other)?);
                }
            }
        }
    }
    let mut skin_materials = HashMap::<u32, u32>::new();
    {
        let mut current_skin = 0u32;
        for line in BufReader::new(sde.by_name("skins.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                current_skin = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
            } else {
                let line = line.trim();
                if let Some(icon_file) = line.strip_prefix("skinMaterialID: ") {
                    skin_materials.insert(current_skin, icon_file.trim().parse().map_err(io::Error::other)?);
                }
            }
        }
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