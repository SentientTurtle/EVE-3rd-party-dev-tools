use evesharedcache::cache::{CacheError, SharedCache};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::{Command, ExitStatusError};
use std::{fs, io};
use std::fs::File;
use std::io::{BufRead, BufReader};
use image::imageops::FilterType;
use image::{imageops, ImageReader};
use image_blend::BufferBlend;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

// Industry "reaction" blueprints use a different background
const REACTION_GROUPS: [u32; 4] = [1888, 1889, 1890, 4097];
// Certain types have 3D models and associated graphicID, but use a 2D icon for their inventory icon
const USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS: [u32; 7] = [12, 340, 448, 548, 649, 711, 4168];

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

pub enum OutputMode {
    Archive
}

fn composite_tech(icon: &Path, tech_icon: &Path, out: &Path, use_magick: bool) -> Result<(), IconError> {
    if use_magick {
        Command::new("magick")
            .arg(icon)
            .arg("-resize").arg("64x64")
            .arg("(").arg(tech_icon).arg("-resize").arg("16x16!").arg(")")   // The tech-tier indicator must be sized; Structure tech tier isn't 16x16 but is squashed as such ingame
            .arg("-composite")
            .arg(out)
            .status()?
            .exit_ok()?;
    } else {
        let mut image = ImageReader::open(icon)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);  // TODO: Consider scaling up the overlay rather than scaling down the image

        let tech_overlay = ImageReader::open(tech_icon)?.with_guessed_format()?.decode()?.resize_exact(16, 16, FilterType::Lanczos3);
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

pub fn build_icon_export<C: SharedCache, P: AsRef<Path>>(outputs: &[OutputMode], data: &IconBuildData, cache: &C, icon_dir: P, force_rebuild: bool, use_magick: bool) -> Result<(usize, usize, usize), IconError> {
    let icon_dir = icon_dir.as_ref();
    fs::create_dir_all(icon_dir)?;

    let mut old_index = HashMap::new();
    let index_path = icon_dir.join("cache.csv");
    if fs::exists(&index_path)? {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(File::open(&index_path)?);
        while reader.read_until(b'\x1E', &mut buf)? > 0 {
            let (path, hash) = std::str::from_utf8(&buf).map_err(io::Error::other)?
                .trim_end_matches('\x1E')
                .split_once('\x1F')
                .ok_or(io::Error::other("malformed index file!"))?;
            old_index.insert(path.to_string(), hash.to_string());
            buf.clear();
        };
    }

    let mut new_index = HashMap::<String, String>::new();
    let mut updated_images = HashSet::<String>::new();

    fn is_up_to_date(old_index: &HashMap<String, String>, new_index: &mut HashMap<String, String>, updated: &mut HashSet<String>, filename: &str, key: &[&str], force_rebuild: bool) -> bool {
        let hash = key.join(";");
        let is_in_index = old_index.get(filename) == Some(&hash);
        new_index.insert(filename.to_string(), hash);
        if !is_in_index { updated.insert(filename.to_string()); }
        is_in_index && !force_rebuild
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
                        let filename = format!("{}_64.png", type_id);
                        // NOTE: single-pipe OR required here as is_up_to_date has essential side effects
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource_bp, techicon], force_rebuild) {
                            composite_tech(&cache.path_of(&*icon_resource_bp)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("{}_64_bpc.png", type_id);
                            if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource_bpc, techicon], force_rebuild) {
                                composite_tech(&cache.path_of(&*icon_resource_bpc)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?;
                            }
                        }
                    } else {
                        let filename = format!("{}_64.png", type_id);
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource_bp], force_rebuild) {
                            fs::copy(cache.path_of(&*icon_resource_bp)?, icon_dir.join(filename))?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("{}_64_bpc.png", type_id);
                            if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource_bpc], force_rebuild) {
                                fs::copy(cache.path_of(&*icon_resource_bpc)?, icon_dir.join(format!("{}_64_bpc.png", type_id)))?;
                            }
                        }
                    }
                }
            } else if let Some(icon) = type_info.icon_id { // If no graphics icon, try icon
                let icon_resource = &*data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?;
                if cache.has_resource(&icon_resource) {
                    let tech_overlay = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1));
                    let filename = format!("{}_64.png", type_id);

                    if category_id == 34 {
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource, "relic", tech_overlay.unwrap_or("")], force_rebuild) {
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
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource, "reaction", tech_overlay.unwrap_or("")], force_rebuild) {
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
                        // BP & BPC BG/overlay
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource, "bpo", tech_overlay.unwrap_or("")], force_rebuild) {
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/bpo.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpo_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                tech_overlay.map(|res| cache.path_of(res)).transpose()?.as_deref(),
                                &icon_dir.join(filename),
                                use_magick
                            )?;
                        }

                        let filename = format!("{}_64_bpc.png", type_id);
                        if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource, "bpc", tech_overlay.unwrap_or("")], force_rebuild) {
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
                    println!("ERR: Missing icon for: {}", type_id);
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

                let filename = format!("{}_512.jpg", type_id);
                let render_resource = format!("{}/{}_512.jpg", folder.trim_end_matches('/'), type_info.graphic_id.unwrap());
                // Intentional short-circuit; If no resource we skip the type as having no icon, and do not update indices
                if cache.has_resource(&*render_resource) && !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*render_resource], force_rebuild) {
                    fs::copy(cache.path_of(&*render_resource)?, icon_dir.join(filename))?;
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
                let filename = format!("{}_64.png", type_id);
                if let Some(techicon) = techicon_resource_for_metagroup(type_info.meta_group_id.unwrap_or(1)) {
                    if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource, techicon], force_rebuild) {
                        composite_tech(&cache.path_of(&icon_resource)?, &cache.path_of(techicon)?, &icon_dir.join(filename), use_magick)?
                    }
                } else {
                    if !is_up_to_date(&old_index, &mut new_index, &mut updated_images, &filename, &[&*icon_resource], force_rebuild) {
                        fs::copy(cache.path_of(&*icon_resource)?, icon_dir.join(filename))?;
                    }
                }
            } else {
                println!("ERR: Missing icon for: {}", type_id);
                continue; // Skip missing icons, sometimes they're broken in-game.
            }
        }
    }

    let index_bytes = new_index.iter()
        .map(|(filename, key)| [filename, "\x1F", key])
        .intersperse(["", "\x1E", ""])  // We want to intersperse between each triplet above, so we add two zero-length strings to create a triplet
        .flatten()
        .flat_map(|str| str.as_bytes())
        .copied()
        .collect::<Vec<u8>>();

    fs::write(index_path, index_bytes)?;

    let to_remove = old_index.keys().filter(|key| !new_index.contains_key(*key)).map(String::as_str).collect::<Vec<&str>>();
    for filename in &to_remove {
        println!("Removing:{}", filename);
        fs::remove_file(icon_dir.join(filename))?;
    }

    let to_add = new_index.keys().filter(|key| !old_index.contains_key(*key)).map(String::as_str).collect::<Vec<&str>>();

    println!("Icons built, generating outputs...");

    for output in outputs {
        match output {
            OutputMode::Archive => {
                // Regenerate bulk zip
                let mut writer = ZipWriter::new(File::create(icon_dir.join("archive.zip"))?);
                for filename in new_index.keys() {
                    // Use stored compression, as image files are already compressed themselves.
                    writer.start_file_from_path(filename, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                    io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                }
                writer.finish().map_err(io::Error::other)?;
            }
        }
    }

    Ok((to_add.len(), updated_images.len().saturating_sub(to_add.len()), to_remove.len()))
}
