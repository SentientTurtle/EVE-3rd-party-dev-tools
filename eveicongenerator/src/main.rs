#![feature(exit_status_error)]

use std::{fs, io};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::Instant;
use evesharedcache::cache::{CacheDownloader};
use zip::ZipArchive;
use crate::icons::{IconBuildData, TypeInfo};
use crate::sde::{download_fsd, get_fsd_checksum};

pub mod icons {
    use std::collections::HashMap;
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::{fs, io};
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitStatusError};
    use evesharedcache::cache::{CacheError, SharedCache};

    // Industry "reaction" blueprints use a different background
    const REACTION_GROUPS: [u32; 4] = [1888, 1889, 1890, 4097];
    // Certain types have 3D models and associated graphicID, but use a 2D icon for their inventory icon
    const USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS: [u32; 6] = [12, 340, 448, 649, 711, 4168];

    pub struct TypeInfo {
        pub group_id: Option<u32>,
        pub icon_id: Option<u32>,
        pub graphic_id: Option<u32>,
        pub meta_group_id: Option<u32>,
    }

    impl Default for TypeInfo {
        fn default() -> Self {
            TypeInfo { group_id: None, icon_id: None, graphic_id: None, meta_group_id: None }
        }
    }

    pub fn techicon_resource_for_metagroup(metagroup_id: u32) -> Option<&'static str> {
        match metagroup_id {
            1 => None,
            2 => Some("res:/ui/texture/icons/73_16_242.png"),
            3 => Some("res:/ui/texture/icons/73_16_245.png"),
            4 => Some("res:/ui/texture/icons/73_16_246.png"),
            5 => Some("res:/ui/texture/icons/73_16_248.png"),
            6 => Some("res:/ui/texture/icons/73_16_247.png"),
            14 => Some("res:/ui/texture/icons/73_16_243.png"),
            15 => Some("res:/ui/texture/icons/itemoverlay/abyssal.png"),
            17 => Some("res:/ui/texture/icons/itemoverlay/nes.png"),
            19 => Some("res:/ui/texture/icons/itemoverlay/timelimited.png"),
            52 => Some("res:/ui/texture/shared/structureoverlayfaction.png"),
            53 => Some("res:/ui/texture/shared/structureoverlayt2.png"),
            54 => Some("res:/ui/texture/shared/structureoverlay.png"),
            _ => None
        }
    }

    #[derive(Debug)]
    pub enum IconError {
        Cache(CacheError),
        IO(io::Error),
        Magick(ExitStatusError),
        String(String)
    }

    impl Display for IconError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                IconError::Cache(err) => Display::fmt(err, f),
                IconError::IO(err) => Display::fmt(err, f),
                IconError::Magick(err) => write!(f, "error in call to image magick {}", err),
                IconError::String(msg) => Display::fmt(msg, f)
            }
        }
    }

    impl Error for IconError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                IconError::Cache(err) => Some(err),
                IconError::IO(err) => Some(err),
                IconError::Magick(err) => Some(err),
                IconError::String(_) => None
            }
        }
    }

    impl From<CacheError> for IconError {
        fn from(value: CacheError) -> Self {
            IconError::Cache(value)
        }
    }

    impl From<io::Error> for IconError {
        fn from(value: io::Error) -> Self {
            IconError::IO(value)
        }
    }

    impl From<ExitStatusError> for IconError {
        fn from(value: ExitStatusError) -> Self {
            IconError::Magick(value)
        }
    }

    pub struct IconBuildData {
        types: Vec<(u32, TypeInfo)>,
        group_categories: HashMap<u32, u32>,
        icon_files: HashMap<u32, String>,
        graphics_folders: HashMap<u32, String>
    }

    impl IconBuildData {
        pub fn new(types: Vec<(u32, TypeInfo)>, group_categories: HashMap<u32, u32>, icon_files: HashMap<u32, String>, graphics_folders: HashMap<u32, String>) -> Self {
            Self { types, group_categories, icon_files, graphics_folders }
        }
    }

    pub fn build_icon_export<C: SharedCache, P: AsRef<Path>>(cache: &C, data: &IconBuildData, icon_dir: P) -> Result<(), IconError> {
        fs::create_dir_all(icon_dir.as_ref())?;
        for (type_id, type_info) in &data.types {
            if type_info.icon_id.is_none() && type_info.graphic_id.is_none() { continue; } // Skip types without iconID or graphicID as they have no icon

            let group_id = match type_info.group_id { Some(id) => id, None => continue };
            let category_id = *data.group_categories.get(&group_id).ok_or_else(|| IconError::String(format!("group without category: {}", group_id)))?;


            if (category_id == 9) || (category_id == 34) {
                // Blueprint or reaction

                if let Some(folder) = type_info.graphic_id.and_then(|graphic_id| data.graphics_folders.get(&graphic_id)) {
                    let icon_resource_bp = format!("{}/{}_64_bp.png", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                    let icon_resource_bpc = format!("{}/{}_64_bpc.png", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());

                    if cache.has_resource(&*icon_resource_bp) && !USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS.contains(&group_id) {
                        if let Some(techicon) = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1)) {
                            Command::new("magick")
                                .arg(cache.path_of(&*icon_resource_bp)?)
                                .arg("-resize").arg("64x64")
                                .arg("(").arg(cache.path_of(techicon)?).arg("-resize").arg("16x16!").arg(")")   // The tech-tier indicator must be sized; Structure tech tier isn't 16x16 but is squashed as such ingame
                                .arg("-composite")
                                .arg(icon_dir.as_ref().join(format!("{}_64.png", type_id)))
                                .status()?
                                .exit_ok()?;

                            if cache.has_resource(&*icon_resource_bpc) {
                                Command::new("magick")
                                    .arg(cache.path_of(&*icon_resource_bpc)?)
                                    .arg("-resize").arg("64x64")
                                    .arg("(").arg(cache.path_of(techicon)?).arg("-resize").arg("16x16!").arg(")")
                                    .arg("-composite")
                                    .arg(icon_dir.as_ref().join(format!("{}_64_bpc.png", type_id)))
                                    .status()?
                                    .exit_ok()?;
                            }
                            continue;
                        } else {
                            fs::copy(cache.path_of(&*icon_resource_bp)?, icon_dir.as_ref().join(format!("{}_64.png", type_id)))?;
                            if cache.has_resource(&*icon_resource_bpc) {
                                fs::copy(cache.path_of(&*icon_resource_bpc)?, icon_dir.as_ref().join(format!("{}_64_bpc.png", type_id)))?;
                            }
                            continue;
                        }
                    }
                }

                // If no graphics icon, try icon
                if let Some(icon) = type_info.icon_id {
                    fn build_command<C: SharedCache>(cache: &C, out_path: PathBuf, background_resource: &str, overlay_resource: &str, icon_resource: &str, tech_overlay: Option<&str>) -> Result<Command, IconError> {
                        let mut command = Command::new("magick");
                        command.arg(cache.path_of(background_resource)?)
                            .arg(cache.path_of(icon_resource)?)
                            .arg("-resize").arg("64x64")
                            .arg("-composite")
                            .arg("-compose").arg("plus")
                            .arg(cache.path_of(overlay_resource)?);

                        if let Some(techicon) = tech_overlay {
                            command.arg("-composite")
                                .arg("-compose").arg("over")
                                .arg("(").arg(cache.path_of(techicon)?).arg("-resize").arg("16x16!").arg(")");
                        }

                        command.arg("-composite").arg(out_path);
                        Ok(command)
                    }

                    let icon_resource = &*data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?;
                    let tech_overlay = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1));

                    if category_id == 34 {
                        // Relic BG/overlay
                        build_command(
                            cache,
                            icon_dir.as_ref().join(format!("{}_64.png", type_id)),
                            "res:/ui/texture/icons/relic.png",
                            "res:/ui/texture/icons/relic_overlay.png",
                            icon_resource,
                            tech_overlay
                        )?
                            .status()?
                            .exit_ok()?;
                        continue;
                    } else if REACTION_GROUPS.contains(&group_id) {
                        // Reaction BG/overlay
                        build_command(
                            cache,
                            icon_dir.as_ref().join(format!("{}_64.png", type_id)),
                            "res:/ui/texture/icons/reaction.png",
                            "res:/ui/texture/icons/bpo_overlay.png", // TODO: Verify against ingame icons
                            icon_resource,
                            tech_overlay
                        )?
                            .status()?
                            .exit_ok()?;
                        continue;
                    } else {
                        // BP & BPC BG/overlay
                        build_command(
                            cache,
                            icon_dir.as_ref().join(format!("{}_64.png", type_id)),
                            "res:/ui/texture/icons/bpo.png",
                            "res:/ui/texture/icons/bpo_overlay.png",
                            icon_resource,
                            tech_overlay
                        )?
                            .status()?
                            .exit_ok()?;

                        build_command(
                            cache,
                            icon_dir.as_ref().join(format!("{}_64_bpc.png", type_id)),
                            "res:/ui/texture/icons/bpc.png",
                            "res:/ui/texture/icons/bpc_overlay.png",
                            icon_resource,
                            tech_overlay
                        )?
                            .status()?
                            .exit_ok()?;
                        continue;
                    }
                } else {
                    continue; // No icon to be generated here
                }
            } else {
                // Regular item

                let graphic_iconinfo = type_info.graphic_id.and_then(|graphic_id| data.graphics_folders.get(&graphic_id));

                let mut icon_resource;
                if let Some(folder) = graphic_iconinfo {
                    icon_resource = format!("{}/{}_64.png", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                    // If no graphic, try icon
                    if !cache.has_resource(&*icon_resource) && !USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS.contains(&group_id) {
                        if let Some(icon) = type_info.icon_id {
                            icon_resource = data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?.clone();
                        } else {
                            continue;   // No icon
                        }
                    }


                    let render_resource = format!("{}/{}_512.jpg", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                    if cache.has_resource(&*render_resource) {
                        fs::copy(cache.path_of(&*render_resource)?, icon_dir.as_ref().join(format!("{}_512.jpg", type_id)))?;
                    }
                } else if let Some(icon) = type_info.icon_id {
                    icon_resource = data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?.clone();
                } else {
                    continue; // No icon to be generated here
                }

                if let Some(techicon) = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1)) {
                    Command::new("magick")
                        .arg(cache.path_of(&*icon_resource)?)
                        .arg("-resize").arg("64x64")
                        .arg("(").arg(cache.path_of(techicon)?).arg("-resize").arg("16x16!").arg(")")
                        .arg("-composite")
                        .arg(icon_dir.as_ref().join(format!("{}_64.png", type_id)))
                        .status()?
                        .exit_ok()?;
                } else {
                    fs::copy(cache.path_of(&*icon_resource)?, icon_dir.as_ref().join(format!("{}_64.png", type_id)))?;
                }
            }
        }

        Ok(())
    }
}

pub mod sde {
    use std::io;
    use std::io::Write;

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
}

#[allow(unused_parens)] // Occasionally over-eager, so disabled for now
fn main() -> Result<(), io::Error> {
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

    let mut fsd = ZipArchive::new(File::open("./cache/fsd.zip")?)?;

    // Parsing the SDEs YAML properly is rather slow and fragile as the SDE is not entirely spec-compliant, so we just directly extract the fields we need.
    println!("\tLoading types...");
    let mut types = HashMap::<u32, TypeInfo>::new();
    {
        let mut current_type: Option<&mut TypeInfo> = None;
        for line in BufReader::new(fsd.by_name("types.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                let type_id = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
                current_type = Some(types.entry(type_id).or_insert_with(TypeInfo::default));
            } else {
                if let Some(ref mut current_type) = current_type {
                    let line = line.trim();
                    if let Some(group_id) = line.strip_prefix("groupID: ") {
                        current_type.group_id = Some(f64::from_str(group_id).map_err(io::Error::other)? as u32); // Some IDs are formatted as float
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
        types.retain(|_, info| info.group_id.is_some() && (info.graphic_id.is_some() || info.icon_id.is_some()));
    }

    println!("\tLoading groups...");
    let mut group_categories = HashMap::<u32, u32>::new();
    {
        let mut current_group = 0u32;
        for line in BufReader::new(fsd.by_name("groups.yaml")?).lines() {
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

    println!("\tLoading iconinfo...");
    let mut icon_files = HashMap::<u32, String>::new();
    {
        let mut current_icon = 0u32;
        for line in BufReader::new(fsd.by_name("iconIDs.yaml")?).lines() {
            let line = line?;
            if line.starts_with(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
                current_icon = u32::from_str(line.trim().trim_end_matches(':')).map_err(io::Error::other)?;
            } else {
                let line = line.trim();
                if let Some(icon_file) = line.strip_prefix("iconFile: ") {
                    icon_files.insert(current_icon, icon_file.trim().to_string());
                }
            }
        }
    }

    println!("\tLoading graphicinfo...");
    let mut graphic_folders = HashMap::<u32, String>::new();
    {
        let mut current_graphic = 0u32;
        for line in BufReader::new(fsd.by_name("graphicIDs.yaml")?).lines() {
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


    println!("Initializing cache");
    let cache = CacheDownloader::initialize("./cache", false).unwrap();

    println!("Building icons...");
    let icon_build_data = IconBuildData::new(
        types.into_iter().collect(),
        group_categories,
        icon_files,
        graphic_folders
    );

    let start = Instant::now();
    icons::build_icon_export(&cache, &icon_build_data, "./icons").unwrap();

    println!("Icons built in: {} seconds", start.elapsed().as_secs_f64());

    // Delete unnecessary cache files to avoid a storage "leak"
    cache.purge(&["fsd.zip", "checksum.txt"])?;

    Ok(())
}
