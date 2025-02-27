use evesharedcache::cache::{CacheError, SharedCache};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::{Command, ExitStatusError};
use std::{fs, io};
use std::io::Write;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use image::imageops::FilterType;
use image::{imageops, ImageFormat, ImageReader};
use image_blend::BufferBlend;
use serde::Serialize;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

// Industry "reaction" blueprints use a different background
const REACTION_GROUPS: [u32; 4] = [1888, 1889, 1890, 4097];
// Certain types have 3D models and associated graphicID, but use a 2D icon for their inventory icon
const USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS: [u32; 8] = [12, 340, 448, 479, 548, 649, 711, 4168];

pub struct TypeInfo {
    pub group_id: u32,
    pub icon_id: Option<u32>,
    pub graphic_id: Option<u32>,
    pub meta_group_id: Option<u32>,
}

impl Default for TypeInfo {
    fn default() -> Self {
        TypeInfo { group_id: 0, icon_id: None, graphic_id: None, meta_group_id: None }
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
    Image(image::ImageError),
    Magick(ExitStatusError),
    String(String)
}

impl Display for IconError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IconError::Cache(err) => Display::fmt(err, f),
            IconError::IO(err) => Display::fmt(err, f),
            IconError::Image(err) => Display::fmt(err, f),
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
            IconError::Image(err) => Some(err),
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

impl From<image::ImageError> for IconError {
    fn from(value: image::ImageError) -> Self {
        IconError::Image(value)
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
    graphics_folders: HashMap<u32, String>,
    skin_materials: HashMap<u32, u32>,
}

impl IconBuildData {
    pub fn new(types: Vec<(u32, TypeInfo)>, group_categories: HashMap<u32, u32>, icon_files: HashMap<u32, String>, graphics_folders: HashMap<u32, String>, skin_materials: HashMap<u32, u32>) -> Self {
        Self { types, group_categories, icon_files, graphics_folders, skin_materials }
    }
}

fn composite_tech(icon: &Path, tech_icon: &Path, out: &Path, use_magick: bool) -> Result<(), IconError> {
    if use_magick {
        Command::new("magick")
            .arg(icon)
            .arg("-resize").arg("64x64")
            .arg("(").arg(tech_icon).arg("-resize").arg("16x16!").arg(")")
            .arg("-composite")
            .arg(out)
            .status()?
            .exit_ok()?;
    } else {
        let mut image = ImageReader::open(icon)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);  // TODO: Consider scaling up the overlay rather than scaling down the image

        let tech_overlay = ImageReader::open(tech_icon)?.with_guessed_format()?.decode()?.resize_exact(16, 16, FilterType::Lanczos3);   // The tech-tier indicator must be sized; Structure tech tier isn't 16x16 but is squashed as such ingame
        imageops::overlay(&mut image, &tech_overlay, 0, 0);

        image.save(out)?;
    }
    Ok(())
}

fn composite_blueprint(background: &Path, overlay: &Path, icon: &Path, tech_icon: Option<&Path>, out: &Path, use_magick: bool) -> Result<(), IconError> {
    if use_magick {
        let mut command = Command::new("magick");
        command.arg(background)
            .arg(icon)
            .arg("-resize").arg("64x64")
            .arg("-composite")
            .arg("-compose").arg("plus")
            .arg(overlay);

        if let Some(icon_path) = tech_icon {
            command.arg("-composite")
                .arg("-compose").arg("over")
                .arg("(").arg(icon_path).arg("-resize").arg("16x16!").arg(")");
        }
        command.arg("-composite").arg(out);
        command.status()?.exit_ok()?;
    } else {
        let mut background_image = ImageReader::open(background)?.with_guessed_format()?.decode()?.into_rgba8();
        let icon_image = ImageReader::open(icon)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);
        imageops::overlay(&mut background_image, &icon_image, 0, 0);
        let overlay_image = ImageReader::open(overlay)?.with_guessed_format()?.decode()?.into_rgba8();

        background_image.blend(&overlay_image, image_blend::pixelops::pixel_add, true, false).map_err(io::Error::other)?;

        if let Some(tech_overlay) = tech_icon {
            let tech_overlay = ImageReader::open(tech_overlay)?.with_guessed_format()?.decode()?.resize_exact(16, 16, FilterType::Lanczos3);
            imageops::overlay(&mut background_image, &tech_overlay, 0, 0);
        }

        background_image.save(out)?;
    }
    Ok(())
}

fn copy_or_convert(from: impl AsRef<Path>, to: impl AsRef<Path>, resource: &str, extension: &str) -> Result<(), IconError> {
    if resource.ends_with(extension) {
        fs::copy(from, to).map(|_| ()).map_err(IconError::from)
    } else {

        let format = match extension {
            ".png" => ImageFormat::Png,
            ".jpg" => ImageFormat::Jpeg,
            ".jpeg" => ImageFormat::Jpeg,
            _ => panic!("Unknown image extension requested: {}", extension)
        };
        ImageReader::open(from)?.with_guessed_format()?.decode()?.save_with_format(to, format).map_err(IconError::from)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize)]
enum IconKind {
    #[serde(rename="icon")]
    Icon,
    #[serde(rename="bp")]
    Blueprint,
    #[serde(rename="bpc")]
    BlueprintCopy,
    #[serde(rename="reaction")]
    Reaction,
    #[serde(rename="relic")]
    Relic,
    #[serde(rename="render")]
    Render
}

impl IconKind {
    pub fn name(self) -> &'static str {
        match self {
            IconKind::Icon => "icon",
            IconKind::Blueprint => "bp",
            IconKind::BlueprintCopy => "bpc",
            IconKind::Reaction => "reaction",
            IconKind::Relic => "relic",
            IconKind::Render => "render"
        }
    }
}

#[derive(Debug)]
pub enum OutputMode<'a> {
    ServiceBundle { out: &'a Path },
    IEC { out: &'a Path },
    Web { out: &'a Path, copy_files: bool, hard_link: bool },
    Checksum { out: Option<&'a Path> }
}

pub fn build_icon_export<C: SharedCache, P: AsRef<Path>>(output_mode: OutputMode, skip_output_if_fresh: bool, data: &IconBuildData, cache: &C, icon_dir: P, force_rebuild: bool, use_magick: bool, silent_mode: bool) -> Result<(usize, usize), IconError> {
    let log_file = crate::LOG_FILE.get();

    let icon_dir = icon_dir.as_ref();
    fs::create_dir_all(icon_dir)?;

    let mut old_index = HashSet::new();
    let index_path = icon_dir.join("cache.csv");
    if fs::exists(&index_path)? {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(File::open(&index_path)?);
        while reader.read_until(b'\x1E', &mut buf)? > 0 {
            let file = std::str::from_utf8(&buf).map_err(io::Error::other)?.trim_end_matches('\x1E');
            old_index.insert(file.to_string());
            buf.clear();
        };
    }

    let mut service_metadata = HashMap::<u32, HashMap<IconKind, String>>::new();
    let mut new_index = HashSet::<String>::new();

    fn is_up_to_date(old_index: &HashSet<String>, new_index: &mut HashSet<String>, filename: &str, force_rebuild: bool) -> bool {
        new_index.insert(filename.to_string());
        old_index.contains(filename) && !force_rebuild
    }

    for (type_id, type_info) in &data.types {
        let category_id = *data.group_categories.get(&type_info.group_id).ok_or_else(|| IconError::String(format!("group without category: {}", type_info.group_id)))?;

        // Skip types without iconID or graphicID as they have no icon, SKINs have custom logic
        if type_info.icon_id.is_none() && type_info.graphic_id.is_none() && category_id != 91 { continue; }

        if (category_id == 9) || (category_id == 34) {
            // Blueprint or reaction

            if let Some(folder) = type_info.graphic_id.and_then(|graphic_id| data.graphics_folders.get(&graphic_id)) {
                let icon_resource_bp = format!("{}/{}_64_bp.png", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                let icon_resource_bpc = format!("{}/{}_64_bpc.png", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());

                if cache.has_resource(&*icon_resource_bp) && !USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS.contains(&type_info.group_id) {
                    if let Some(techicon) = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1)) {
                        let filename = format!("bp;{};{}.png", cache.hash_of(&icon_resource_bp)?, cache.hash_of(techicon)?);
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            composite_tech(&cache.path_of(&*icon_resource_bp)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("bpc;{};{}.png", cache.hash_of(&icon_resource_bpc)?, cache.hash_of(techicon)?);
                            service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                            if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                                composite_tech(&cache.path_of(&*icon_resource_bpc)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?;
                            }
                        }
                    } else {
                        let filename = format!("bp;{}.png", cache.hash_of(&icon_resource_bp)?);
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            copy_or_convert(cache.path_of(&*icon_resource_bp)?, icon_dir.join(filename), &*icon_resource_bp, ".png")?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("bpc;{}.png", cache.hash_of(&icon_resource_bpc)?);
                            service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                            if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                                copy_or_convert(cache.path_of(&*icon_resource_bpc)?, icon_dir.join(filename), &*icon_resource_bp, ".png")?;
                            }
                        }
                    }
                }
            } else if let Some(icon) = type_info.icon_id { // If no graphics icon, try icon
                let icon_resource = &*data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?;
                if cache.has_resource(&icon_resource) {
                    let tech_overlay = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1));

                    if category_id == 34 {
                        let filename = format!("relic;{};{}.png", cache.hash_of(icon_resource)?, tech_overlay.map(|res| cache.hash_of(res)).transpose()?.unwrap_or(""));

                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Relic, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            // Relic BG/overlay
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/relic.png")?,
                                &cache.path_of("res:/ui/texture/icons/relic_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                tech_overlay.map(|res| cache.path_of(res)).transpose()?.as_deref(),
                                &icon_dir.join(filename),
                                use_magick
                            )?;
                        }
                    } else if REACTION_GROUPS.contains(&type_info.group_id) {
                        let filename = format!("reaction;{};{}.png", cache.hash_of(icon_resource)?, tech_overlay.map(|res| cache.hash_of(res)).transpose()?.unwrap_or(""));

                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Reaction, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());   // Incorrect behaviour of the image service, included for compatibility
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            // Reaction BG/overlay
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/reaction.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpo_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                tech_overlay.map(|res| cache.path_of(res)).transpose()?.as_deref(),
                                &icon_dir.join(filename),
                                use_magick
                            )?;
                        }
                    } else {
                        let filename = format!("bp;{};{}.png", cache.hash_of(icon_resource)?, tech_overlay.map(|res| cache.hash_of(res)).transpose()?.unwrap_or(""));

                        // BP & BPC BG/overlay
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/bpo.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpo_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                tech_overlay.map(|res| cache.path_of(res)).transpose()?.as_deref(),
                                &icon_dir.join(filename),
                                use_magick
                            )?;
                        }

                        let filename = format!("bpc;{};{}.png", cache.hash_of(icon_resource)?, tech_overlay.map(|res| cache.hash_of(res)).transpose()?.unwrap_or(""));
                        service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/bpc.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpc_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                tech_overlay.map(|res| cache.path_of(res)).transpose()?.as_deref(),
                                &icon_dir.join(filename),
                                use_magick
                            )?;
                        }
                    }
                } else {
                    // Skip missing icons, sometimes they're broken in-game.
                    if !silent_mode { println!("\tERR: Missing icon for: {}", type_id); }
                    if let Some(mut log) = log_file { writeln!(log, "\tERR: Missing icon for: {}", type_id)?; }
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
                if !cache.has_resource(&*icon_resource) || USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS.contains(&type_info.group_id) {
                    if let Some(icon) = type_info.icon_id {
                        icon_resource = data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?.clone();
                    } else {
                        continue;   // No icon
                    }
                }

                let render_resource = format!("{}/{}_512.jpg", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                if cache.has_resource(&*render_resource) {
                    let filename = format!("{}.jpg", cache.hash_of(&render_resource)?);
                    service_metadata.entry(*type_id).or_default().insert(IconKind::Render, filename.clone());
                    if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                        copy_or_convert(cache.path_of(&*render_resource)?, icon_dir.join(filename), &*render_resource, ".jpg")?;
                    }
                }
            } else if let Some(icon) = type_info.icon_id {
                icon_resource = data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?.clone();
            } else if category_id == 91 {
                // SKIN
                if let Some(material_id) = data.skin_materials.get(type_id) {
                    icon_resource = format!("res:/ui/texture/classes/skins/icons/{}.png", material_id);
                } else {
                    continue;   // Some skins are region-exclusive and do not have the resources available on the TQ client, so skip and treat as no-icon types
                }
            } else {
                continue; // No icon to be generated here
            }

            if cache.has_resource(&icon_resource) {
                if let Some(techicon) = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1)) {
                    let filename = format!("{};{}.png", cache.hash_of(&*icon_resource)?, cache.hash_of(techicon)?);
                    service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());

                    if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                        composite_tech(&cache.path_of(&icon_resource)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?
                    }
                } else {
                    let filename = format!("{}.png", cache.hash_of(&*icon_resource)?);
                    service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());

                    if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                        copy_or_convert(cache.path_of(&*icon_resource)?, icon_dir.join(filename), &*icon_resource, ".png")?;
                    }
                }
            } else {
                if !silent_mode { println!("\tERR: Missing icon for: {}", type_id); }
                if let Some(mut log) = log_file { writeln!(log, "\tERR: Missing icon for: {}", type_id)?; }
                continue; // Skip missing icons, sometimes they're broken in-game.
            }
        }
    }

    let mut sort_index = Vec::with_capacity(new_index.len());
    new_index.iter().map(String::as_str).collect_into(&mut sort_index);
    sort_index.sort();

    let index_bytes = sort_index.into_iter()
        .intersperse("\x1E")
        .flat_map(|str| str.as_bytes())
        .copied()
        .collect::<Vec<u8>>();

    fs::write(index_path, &index_bytes)?;

    let to_remove = old_index.iter().filter(|key| !new_index.contains(*key)).map(String::as_str).collect::<Vec<&str>>();
    let to_add = new_index.iter().filter(|key| !old_index.contains(*key)).map(String::as_str).collect::<Vec<&str>>();

    if to_add.len() == 0 && to_remove.len() == 0 && skip_output_if_fresh {
        if !silent_mode { println!("Icons fresh, skipping outputs..."); }
        if let Some(mut log) = log_file { writeln!(log, "Icons fresh, skipping outputs...")?; }
    } else {
        if !silent_mode { println!("Icons built, generating outputs..."); }
        if let Some(mut log) = log_file { writeln!(log, "Icons built, generating outputs...")?; }
        match output_mode {
            OutputMode::ServiceBundle { out} => {
                if let Some(mut log) = log_file { writeln!(log, "Writing Service Bundle to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);
                for filename in &new_index {
                    writer.start_file(filename, FileOptions::<()>::default().compression_method(CompressionMethod::Stored))
                        .map_err(|e| format!("err in {}: {}", filename, e))
                        .map_err(io::Error::other)?;
                    if let Some(mut log) = log_file { writeln!(log, "\t{}", filename)?; }
                    io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                }

                writer.start_file("service_metadata.json", FileOptions::<()>::default()).map_err(io::Error::other)?;
                serde_json::to_writer_pretty(&mut writer, &service_metadata).map_err(io::Error::other)?;

                writer.finish().map_err(io::Error::other)?;
            }
            OutputMode::IEC { out } => {
                if let Some(mut log) = log_file { writeln!(log, "Writing IEC archive to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);
                // Copy the icons IEC-style; Types with the same icon get duplicated files
                for (type_id, icons) in &service_metadata {
                    for (icon_kind, filename) in icons {
                        match icon_kind {
                            IconKind::Icon => {
                                let output_name = format!("{}_64.png", type_id);
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                if let Some(mut log) = log_file { writeln!(log, "\t{} as {}", filename, output_name)?; }
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                            IconKind::Blueprint | IconKind::Reaction | IconKind::Relic => { /* None, these are duplicated by IconKind::Icon */}
                            IconKind::BlueprintCopy => {
                                let output_name = format!("{}_bpc_64.png", type_id);
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                if let Some(mut log) = log_file { writeln!(log, "\t{} as {}", filename, output_name)?; }
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                            IconKind::Render => {
                                let output_name = format!("{}_512.jpg", type_id);
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                if let Some(mut log) = log_file { writeln!(log, "\t{} as {}", filename, output_name)?; }
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                        }
                    }
                }
                writer.finish().map_err(io::Error::other)?;
            }
            OutputMode::Web { out, copy_files, hard_link } => {
                let mode_name = if copy_files { "COPYING" } else if hard_link { "HARD LINK" } else { "SOFT LINK" };
                if let Some(mut log) = log_file { writeln!(log, "Building web folder to {:?} ({})", out, mode_name)?; }
                let mut created_files = HashMap::<String, String>::new();

                let index_path = out.join("index.json");
                let old_links = if fs::exists(&index_path)? {
                     serde_json::from_reader::<_, HashMap<String, String>>(File::open(&index_path)?).map_err(io::Error::other)?
                } else {
                    HashMap::new()
                };

                let mut kind_buf = Vec::<IconKind>::new();
                for (type_id, icons) in &service_metadata {
                    let json_name = format!("{}.json", type_id);
                    let json_filename = out.join(&json_name);
                    icons.keys().collect_into(&mut kind_buf);
                    let json_content = serde_json::to_string(&kind_buf).map_err(io::Error::other)?;
                    kind_buf.clear();
                    if force_rebuild || old_links.get(&json_name) != Some(&json_content) {
                        fs::write(&json_filename, json_content.as_bytes())?;
                    }
                    created_files.insert(json_name, json_content);

                    for (icon_kind, filename) in icons {
                        let link_name = format!("{}_{}.{}", type_id, icon_kind.name(), if IconKind::Render == *icon_kind { "jpg" } else { "png" });
                        let link_source = std::path::absolute(icon_dir.join(filename))?;
                        let link_file = std::path::absolute(out.join(&link_name))?;

                        if force_rebuild || old_links.get(&link_name) != Some(&filename) {
                            if let Some(mut log) = log_file { writeln!(log, "\t{} -> {}", &filename, &link_name)?; }
                            if copy_files {
                                fs::copy(link_source, link_file)?;
                            } else if hard_link {
                                if fs::exists(&link_file)? { fs::remove_file(&link_file)? };
                                fs::hard_link(link_source, link_file)?;
                            } else {
                                if fs::exists(&link_file)? { fs::remove_file(&link_file)? };
                                #[cfg(target_family = "windows")]
                                std::os::windows::fs::symlink_file(link_source, link_file)?;
                                #[cfg(target_family = "unix")]
                                std::os::unix::fs::symlink(link_source, link_file)?;
                                #[cfg(not(any(target_family = "windows", target_family = "unix")))]
                                compile_error!("Can't create symlink on OS that is neither windows nor unix :(")
                            }
                        } else {
                            if let Some(mut log) = log_file { writeln!(log, "\tSKIP: {}", &link_name)?; }
                        }
                        created_files.insert(link_name, filename.clone());
                    }
                }

                for entry in old_links.keys() {
                    if !created_files.contains_key(entry) {
                        if let Some(mut log) = log_file { writeln!(log, "\tRemoved: {}", &entry)?; }
                        match fs::remove_file(out.join(entry)) {
                            Ok(()) => Ok(()),
                            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                            res => res
                        }?;
                    }
                }
                serde_json::to_writer(File::create(&index_path)?, &created_files).map_err(io::Error::other)?;
            }
            OutputMode::Checksum { out: Some(outfile) } => {
                fs::write(outfile, format!("{:x}", md5::compute(&index_bytes)))?;
            }
            OutputMode::Checksum { out: None } => print!("{:x}", md5::compute(&index_bytes)),
        }
    }

    for filename in &to_remove {
        fs::remove_file(icon_dir.join(filename))?;
    }

    Ok((to_add.len(), to_remove.len()))
}
