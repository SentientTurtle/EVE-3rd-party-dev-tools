use evesharedcache::cache::{CacheError, SharedCache};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::{fs, io};
use std::io::{Cursor, Write};
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use evestaticdata::sde::load::{SDELoadError, SDELoader};
use evestaticdata::types::{ids, values};
use image::imageops::FilterType;
use image::{imageops, DynamicImage, ImageFormat, ImageReader};
use image_blend::BufferBlend;
use serde::{Deserialize, Serialize};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

// Industry "reaction" blueprints use a different background
const REACTION_GROUPS: [u32; 4] = [1888, 1889, 1890, 4097];
// Certain types have 3D models and associated graphicID, but use a 2D icon for their inventory icon
const USE_ICON_INSTEAD_OF_GRAPHIC_GROUPS: [u32; 8] = [12, 340, 448, 479, 548, 649, 711, 4168];

#[derive(Debug, Copy, Clone)]
pub struct IconConfig {
    pub use_old_overlays: bool,
    pub module_overlays: bool,
    pub clone_overlays: bool
}

#[derive(Deserialize)]
pub struct TypeInfo {
    #[serde(rename="groupID")]
    pub group_id: u32,
    #[serde(rename="iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename="graphicID")]
    pub graphic_id: Option<u32>,
    #[serde(rename="metaGroupID")]
    pub meta_group_id: Option<u32>,
    #[serde(skip)]
    pub module_slot: Option<ModuleSlot>,
    #[serde(skip)]
    pub omega_required: Option<bool>
}

impl Default for TypeInfo {
    fn default() -> Self {
        TypeInfo { group_id: 0, icon_id: None, graphic_id: None, meta_group_id: None, module_slot: None, omega_required: None }
    }
}

#[derive(Copy, Clone)]
pub enum IconOverlay {  // TODO: Cache parsed images
    None,
    Resource(&'static str),
    Bytes(&'static [u8], &'static str)
}

impl IconOverlay {
    pub fn load<C: SharedCache>(self, cache: &C) -> Result<Option<(&str, DynamicImage)>, IconError> {
        match self {
            IconOverlay::None => Ok(None),
            IconOverlay::Resource(res) => Ok(Some((res.rsplit_once('/').map(|(_, suffix)| suffix).expect("resources are hardcoded and always have a filename"), ImageReader::open(cache.path_of(res)?)?.with_guessed_format()?.decode()?.resize_exact(16, 16, FilterType::Lanczos3)))),
            IconOverlay::Bytes(bytes, name) => {
                let mut reader = ImageReader::new(Cursor::new(bytes));
                reader.set_format(ImageFormat::Png);
                Ok(Some((name, reader.decode()?.resize_exact(16, 16, FilterType::Lanczos3))))
            }
        }
    }
}

pub fn get_techoverlay(metagroup_id: u32, use_old_style: bool) -> IconOverlay {
    if use_old_style {
        match metagroup_id {
            1 => IconOverlay::None,
            2 => IconOverlay::Bytes(include_bytes!("./rsc/Tech 2.png"), "t2-old"),
            3 => IconOverlay::Bytes(include_bytes!("./rsc/Storyline.png"), "storyline-old"),
            4 => IconOverlay::Bytes(include_bytes!("./rsc/Faction.png"), "faction-old"),
            5 => IconOverlay::Bytes(include_bytes!("./rsc/Officer.png"), "officer-old"),
            6 => IconOverlay::Bytes(include_bytes!("./rsc/Deadspace.png"), "deadspace-old"),
            14 => IconOverlay::Bytes(include_bytes!("./rsc/Tech 3.png"), "t3-old"),
            15 => IconOverlay::Bytes(include_bytes!("./rsc/Abyssal.png"), "abyssal-old"),
            17 => IconOverlay::Bytes(include_bytes!("./rsc/NES.png"), "nes-old"),
            19 => IconOverlay::Bytes(include_bytes!("./rsc/Time Limited.png"), "timelimited-old"),
            52 => IconOverlay::Bytes(include_bytes!("./rsc/Structure Faction.png"), "structurefaction-old"),
            53 => IconOverlay::Bytes(include_bytes!("./rsc/Structure Tech 2.png"), "structuret2-old"),
            54 => IconOverlay::Bytes(include_bytes!("./rsc/Structure Tech 1.png"), "structuret1-old"),
            _ => IconOverlay::None
        }
    } else {
        match metagroup_id {
            1 => IconOverlay::None,
            2 => IconOverlay::Resource("res:/ui/texture/icons/73_16_242.png"),
            3 => IconOverlay::Resource("res:/ui/texture/icons/73_16_245.png"),
            4 => IconOverlay::Resource("res:/ui/texture/icons/73_16_246.png"),
            5 => IconOverlay::Resource("res:/ui/texture/icons/73_16_248.png"),
            6 => IconOverlay::Resource("res:/ui/texture/icons/73_16_247.png"),
            14 => IconOverlay::Resource("res:/ui/texture/icons/73_16_243.png"),
            15 => IconOverlay::Resource("res:/ui/texture/icons/itemoverlay/abyssal.png"),
            17 => IconOverlay::Resource("res:/ui/texture/icons/itemoverlay/nes.png"),
            19 => IconOverlay::Resource("res:/ui/texture/icons/itemoverlay/timelimited.png"),
            52 => IconOverlay::Resource("res:/ui/texture/shared/structureoverlayfaction.png"),
            53 => IconOverlay::Resource("res:/ui/texture/shared/structureoverlayt2.png"),
            54 => IconOverlay::Resource("res:/ui/texture/shared/structureoverlay.png"),
            _ => IconOverlay::None
        }
    }
}

pub fn get_moduleoverlay(module_slot: Option<ModuleSlot>, _use_old_style: bool) -> IconOverlay {
    // Module overlays currently have no new style, so always use the old style
    match module_slot {
        None => IconOverlay::None,
        Some(ModuleSlot::High) => IconOverlay::Bytes(include_bytes!("./rsc/Slot-High.png"), "slot-hi-old"),
        Some(ModuleSlot::Medium) => IconOverlay::Bytes(include_bytes!("./rsc/Slot-Med.png"), "slot-me-old"),
        Some(ModuleSlot::Low) => IconOverlay::Bytes(include_bytes!("./rsc/Slot-Low.png"), "slot-lo-old"),
        Some(ModuleSlot::Rig) => IconOverlay::Bytes(include_bytes!("./rsc/Slot-Rig.png"), "slot-ri-old"),
        Some(ModuleSlot::Subsystem) => IconOverlay::Bytes(include_bytes!("./rsc/Slot-Subsystem.png"), "slot-su-old")
    }
}

pub fn get_cloneoverlay(requires_omega: Option<bool>, use_old_style: bool) -> IconOverlay {
    if use_old_style {
        match requires_omega {
            None => IconOverlay::None,
            Some(true) => IconOverlay::Bytes(include_bytes!("./rsc/Omega-Old.png"), "omega-old"),
            Some(false) => IconOverlay::Bytes(include_bytes!("./rsc/Alpha-Old.png"), "alpha-old")
        }
    } else {
        match requires_omega {
            None => IconOverlay::None,
            Some(true) => IconOverlay::Bytes(include_bytes!("./rsc/Omega-New.png"), "omega-new"),
            Some(false) => IconOverlay::Bytes(include_bytes!("./rsc/Alpha-New.png"), "alpha-new")
        }
    }
}

#[derive(Debug)]
pub enum IconError {
    Cache(CacheError),
    SDE(SDELoadError),
    IO(io::Error),
    Image(image::ImageError),
    String(String)
}

impl Display for IconError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IconError::Cache(err) => Display::fmt(err, f),
            IconError::SDE(err) => Display::fmt(err, f),
            IconError::IO(err) => Display::fmt(err, f),
            IconError::Image(err) => Display::fmt(err, f),
            IconError::String(msg) => Display::fmt(msg, f),
        }
    }
}

impl Error for IconError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            IconError::Cache(err) => Some(err),
            IconError::SDE(err) => Some(err),
            IconError::IO(err) => Some(err),
            IconError::Image(err) => Some(err),
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

impl From<SDELoadError> for IconError {
    fn from(value: SDELoadError) -> Self {
        IconError::SDE(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ModuleSlot {
    High,
    Medium,
    Low,
    Rig,
    Subsystem
}

pub struct IconBuildData {
    types: HashMap<u32, TypeInfo>,
    group_categories: HashMap<u32, u32>,
    icon_files: HashMap<u32, String>,
    graphics_folders: HashMap<u32, String>,
    skin_materials: HashMap<u32, u32>
}

impl IconBuildData {
    pub fn load(mut loader: SDELoader, icon_config: IconConfig) -> Result<IconBuildData, SDELoadError> {
        let group_categories = { loader.load_groups()?.map(|g_res| g_res.map(|g| (g.groupID, g.categoryID))).collect::<Result<HashMap<_, _>, _>>()? };

        Ok(Self {
            types: {
                let mut types = HashMap::<ids::TypeID, TypeInfo>::new();
                for item_type in loader.load_types()? {
                    let item_type = item_type?;
                    if item_type.graphicID.is_some() || item_type.iconID.is_some() || (1950..=1955).contains(&item_type.groupID) || item_type.groupID == 4040 {
                        types.insert(item_type.typeID, TypeInfo {
                            group_id: item_type.groupID,
                            icon_id: item_type.iconID,
                            graphic_id: item_type.graphicID,
                            meta_group_id: item_type.metaGroupID,
                            module_slot: None,
                            omega_required: None,
                        });
                    }
                }

                if icon_config.clone_overlays || icon_config.module_overlays {
                    let mut alpha_skills = HashMap::<ids::TypeID, values::SkillLevel>::new();
                    if icon_config.clone_overlays {
                        for clone_grade in loader.load_clone_grades()? {
                            let clone_grade = clone_grade?;
                            alpha_skills.extend(clone_grade.skills);
                            break;  // All alpha clone grades are the same, skip after the first
                        }

                        for (skill, _) in &alpha_skills {
                            if let Some(item_type) = types.get_mut(skill) {
                                item_type.omega_required = Some(false)
                            }
                        }
                    }

                    for type_dogma in loader.load_type_dogma()? {
                        let type_dogma = type_dogma?;
                        if let Some(item_type) = types.get_mut(&type_dogma.typeID) {

                            if type_dogma.dogmaEffects.contains_key(&11) { item_type.module_slot = Some(ModuleSlot::Low); }
                            if type_dogma.dogmaEffects.contains_key(&13) { item_type.module_slot = Some(ModuleSlot::Medium); }
                            if type_dogma.dogmaEffects.contains_key(&12) { item_type.module_slot = Some(ModuleSlot::High); }
                            if type_dogma.dogmaEffects.contains_key(&2663) { item_type.module_slot = Some(ModuleSlot::Rig); }
                            if type_dogma.dogmaEffects.contains_key(&3772) { item_type.module_slot = Some(ModuleSlot::Subsystem); }

                            if icon_config.clone_overlays {
                                // For skills, set requirement directly. For other items, determine requirement from required skills
                                if group_categories.get(&item_type.group_id) == Some(&16) {
                                    item_type.omega_required = Some(!alpha_skills.contains_key(&type_dogma.typeID))
                                } else {
                                    const SKILL_ATTRIBUTES: [ids::AttributeID; 6] = [182, 183, 184, 1285, 1289, 1290];
                                    const LEVEL_ATTRIBUTES: [ids::AttributeID; 6] = [277, 278, 279, 1286, 1287, 1288];

                                    let mut skill_required = false;
                                    let mut omega_required = false;

                                    for i in 0..6 {
                                        if let (Some(skill), Some(level)) = (type_dogma.dogmaAttributes.get(&SKILL_ATTRIBUTES[i]), type_dogma.dogmaAttributes.get(&LEVEL_ATTRIBUTES[i])) {
                                            skill_required = true;
                                            omega_required |= alpha_skills.get(&(*skill as ids::TypeID)).is_none_or(|alpha_level| *alpha_level < (*level as u8));
                                        }
                                    }

                                    if skill_required {
                                        item_type.omega_required = Some(omega_required)
                                    }
                                }
                            }
                        }
                    }
                }

                types
            },
            group_categories,
            icon_files: { loader.load_icons()?.map(|i_res| i_res.map(|i| (i.iconID, i.iconFile))).collect::<Result<HashMap<_, _>, _>>()? },
            graphics_folders: {
                loader.load_graphics()?.flat_map(|g_res| {
                    match g_res {
                        Ok(graphic) => graphic.iconFolder.map(|file| Ok((graphic.graphicID, file))),
                        Err(err) => Some(Err(err))
                    }
                })
                    .collect::<Result<HashMap<u32, String>, SDELoadError>>()?
            },
            skin_materials: {
                let license_skins = loader.load_skin_licenses()?.map(|l_res| l_res.map(|l| (l.typeID, l.skinID))).collect::<Result<HashMap<_, _>, _>>()?;
                let skin_materials = loader.load_skins()?.map(|s_res| s_res.map(|s| (s.skinID, s.skinMaterialID))).collect::<Result<HashMap<_, _>, _>>()?;

                let mut license_materials = HashMap::with_capacity(license_skins.len());
                for (license_id, skin_id) in license_skins {
                    if let Some(material_id) = skin_materials.get(&skin_id) {
                        license_materials.insert(license_id, *material_id);
                    }
                }
                license_materials
            }
        })
    }
}

fn composite_blueprint(background: &Path, overlay: &Path, icon: &Path, tech_icon: Option<&DynamicImage>, out: &Path) -> Result<(), IconError> {
    let mut background_image = ImageReader::open(background)?.with_guessed_format()?.decode()?.into_rgba8();
    let icon_image = ImageReader::open(icon)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);
    imageops::overlay(&mut background_image, &icon_image, 0, 0);
    let overlay_image = ImageReader::open(overlay)?.with_guessed_format()?.decode()?.into_rgba8();

    background_image.blend(&overlay_image, image_blend::pixelops::pixel_add, true, false).map_err(io::Error::other)?;

    if let Some(techoverlay) = tech_icon {
        imageops::overlay(&mut background_image, techoverlay, 0, 0);
    }

    background_image.save(out)?;
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
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

impl Display for IconKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <str as Display>::fmt(self.name(), f)
    }
}

#[derive(Debug)]
pub enum OutputMode<'a> {
    ServiceBundle { out: &'a Path },
    IEC { out: &'a Path },
    Web { out: &'a Path, copy_files: bool, hard_link: bool },
    Checksum { out: Option<&'a Path> },
    AuxIcons { out: &'a Path },
    AuxImages { out: &'a Path, incl_character: bool }
}

pub fn build_icon_export<C: SharedCache, P: AsRef<Path>>(icon_config: IconConfig, output_modes: Vec<OutputMode>, skip_output_if_fresh: bool, no_purge: bool, data: &IconBuildData, cache: &C, icon_dir: P, force_rebuild: bool, mut silent_mode: bool) -> Result<(usize, usize), IconError> {
    let log_file = crate::LOG_FILE.get();   // TODO: Put in a parameter
    silent_mode |= output_modes.iter().any(|mode| matches!(mode, OutputMode::Checksum { out: None }));  // If "Checksum to stdout" output mode is present, enforce silent mode

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

    let mut service_metadata = BTreeMap::<u32, BTreeMap<IconKind, String>>::new();
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
                    let techoverlay = get_techoverlay(type_info.meta_group_id.unwrap_or(1), icon_config.use_old_overlays);
                    if let Some((overlay_name, techoverlay)) = techoverlay.load(cache)? {
                        let filename = format!("bp;{};{}.png", cache.hash_of(&icon_resource_bp)?, overlay_name);
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            let mut image = ImageReader::open(&cache.path_of(&*icon_resource_bp)?)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);
                            imageops::overlay(&mut image, &techoverlay, 0, 0);
                            image.save(&icon_dir.join(filename))?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("bpc;{};{}.png", cache.hash_of(&icon_resource_bpc)?, overlay_name);
                            service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                            if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                                let mut image = ImageReader::open(&cache.path_of(&*icon_resource_bpc)?)?.with_guessed_format()?.decode()?
                                    .resize_exact(64, 64, FilterType::Lanczos3);
                                imageops::overlay(&mut image, &techoverlay, 0, 0);
                                image.save(&icon_dir.join(filename))?;
                            }
                        }
                    } else {
                        let filename = format!("bp;{}.png", cache.hash_of(&icon_resource_bp)?);
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            let image = ImageReader::open(&cache.path_of(&*icon_resource_bp)?)?.with_guessed_format()?.decode()?
                                .resize_exact(64, 64, FilterType::Lanczos3);
                            image.save(&icon_dir.join(filename))?;
                        }

                        if cache.has_resource(&*icon_resource_bpc) {
                            let filename = format!("bpc;{}.png", cache.hash_of(&icon_resource_bpc)?);
                            service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                            if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                                let image = ImageReader::open(&cache.path_of(&*icon_resource_bpc)?)?.with_guessed_format()?.decode()?
                                    .resize_exact(64, 64, FilterType::Lanczos3);
                                image.save(&icon_dir.join(filename))?;
                            }
                        }
                    }
                }
            } else if let Some(icon) = type_info.icon_id { // If no graphics icon, try icon
                let icon_resource = &*data.icon_files.get(&icon).ok_or(IconError::String(format!("unknown icon id: {}", icon)))?;
                if cache.has_resource(&icon_resource) {
                    let (techoverlay_name, techoverlay) = get_techoverlay(type_info.meta_group_id.unwrap_or(1), icon_config.use_old_overlays).load(cache)?.unzip();

                    if category_id == 34 {
                        let filename = format!("relic;{};{}.png", cache.hash_of(icon_resource)?, techoverlay_name.unwrap_or(""));

                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Relic, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            // Relic BG/overlay
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/relic.png")?,
                                &cache.path_of("res:/ui/texture/icons/relic_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                techoverlay.as_ref(),
                                &icon_dir.join(filename)
                            )?;
                        }
                    } else if REACTION_GROUPS.contains(&type_info.group_id) {
                        let filename = format!("reaction;{};{}.png", cache.hash_of(icon_resource)?, techoverlay_name.unwrap_or(""));

                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Reaction, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());   // Incorrect behaviour of the image service, included for compatibility
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            // Reaction BG/overlay
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/reaction.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpo_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                techoverlay.as_ref(),
                                &icon_dir.join(filename)
                            )?;
                        }
                    } else {
                        let filename = format!("bp;{};{}.png", cache.hash_of(icon_resource)?, techoverlay_name.unwrap_or(""));

                        // BP & BPC BG/overlay
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());
                        service_metadata.entry(*type_id).or_default().insert(IconKind::Blueprint, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/bpo.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpo_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                techoverlay.as_ref(),
                                &icon_dir.join(filename)
                            )?;
                        }

                        let filename = format!("bpc;{};{}.png", cache.hash_of(icon_resource)?, techoverlay_name.unwrap_or(""));
                        service_metadata.entry(*type_id).or_default().insert(IconKind::BlueprintCopy, filename.clone());
                        if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                            composite_blueprint(
                                &cache.path_of("res:/ui/texture/icons/bpc.png")?,
                                &cache.path_of("res:/ui/texture/icons/bpc_overlay.png")?,
                                &cache.path_of(icon_resource)?,
                                techoverlay.as_ref(),
                                &icon_dir.join(filename)
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
                        let _ = fs::copy(cache.path_of(&*render_resource)?, icon_dir.join(filename)).map_err(IconError::from)?;
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

            if !cache.has_resource(&icon_resource) {
                if !silent_mode { println!("\tERR: Missing icon for: {}", type_id); }
                if let Some(mut log) = log_file { writeln!(log, "\tERR: Missing icon for: {}", type_id)?; }
                continue; // Skip missing icons, sometimes they're broken in-game.
            }

            let techoverlay = get_techoverlay(type_info.meta_group_id.unwrap_or(1), icon_config.use_old_overlays).load(cache)?;
            let moduleoverlay = get_moduleoverlay(type_info.module_slot, icon_config.use_old_overlays).load(cache)?;
            let cloneoverlay = get_cloneoverlay(type_info.omega_required, icon_config.use_old_overlays).load(cache)?;

            if let (None, None, None) = (&techoverlay, &moduleoverlay, &cloneoverlay) {
                let filename = format!("{}.png", cache.hash_of(&*icon_resource)?);
                service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());

                if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                    let image = ImageReader::open(&cache.path_of(&*icon_resource)?)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);
                    image.save(&icon_dir.join(filename))?;
                }
            } else {
                let (techoverlay_name, techoverlay) = techoverlay.unzip();
                let (moduleoverlay_name, moduleoverlay) = moduleoverlay.unzip();
                let (cloneoverlay_name, cloneoverlay) = cloneoverlay.unzip();

                let filename = format!("{};{};{};{}.png", cache.hash_of(&*icon_resource)?, techoverlay_name.unwrap_or(""), moduleoverlay_name.unwrap_or(""), cloneoverlay_name.unwrap_or(""));
                service_metadata.entry(*type_id).or_default().insert(IconKind::Icon, filename.clone());

                if !is_up_to_date(&old_index, &mut new_index, &filename, force_rebuild) {
                    let mut image = ImageReader::open(&cache.path_of(&icon_resource)?)?.with_guessed_format()?.decode()?.resize_exact(64, 64, FilterType::Lanczos3);
                    if let Some(techoverlay) = techoverlay {
                        imageops::overlay(&mut image, &techoverlay, 0, 0);
                    }
                    if let Some(moduleoverlay) = moduleoverlay {
                        imageops::overlay(&mut image, &moduleoverlay, 48, 48);
                    }
                    if let Some(cloneoverlay) = cloneoverlay {
                        imageops::overlay(&mut image, &cloneoverlay, 48, 0);
                    }
                    image.save(&icon_dir.join(filename))?;
                }
            }
        }
    }

    let mut sort_index = new_index.iter().map(String::as_str).collect::<Vec<_>>();
    sort_index.sort();

    let mut index_bytes = Vec::new();

    let mut first = true;
    for item in sort_index {
        if first {
            first = false;
        } else {
            index_bytes.extend(b"\x1E");
        }
        index_bytes.extend(item.as_bytes())
    }

    fs::write(index_path, &index_bytes)?;

    let to_remove = old_index.iter().filter(|key| !new_index.contains(*key)).map(String::as_str).collect::<Vec<&str>>();
    let to_add = new_index.iter().filter(|key| !old_index.contains(*key)).map(String::as_str).collect::<Vec<&str>>();

    let skip_output = to_add.len() == 0 && to_remove.len() == 0 && skip_output_if_fresh;
    if skip_output {
        if !silent_mode { println!("Icons fresh, skipping output..."); }
        if let Some(mut log) = log_file { writeln!(log, "Icons fresh, skipping output...")?; }
    } else {
        if !silent_mode { println!("Icons built, generating output..."); }
        if let Some(mut log) = log_file { writeln!(log, "Icons built, generating output...")?; }
    }

    for output_mode in output_modes {
        match output_mode {
            OutputMode::ServiceBundle { out } => {
                if skip_output {
                    if !silent_mode { println!("\tSKIPPED Service Bundle"); }
                    if let Some(mut log) = log_file { writeln!(log, "\tSKIPPED Service Bundle")?; }
                    continue;
                }

                if !silent_mode { println!("\tWriting Service Bundle to {:?}", out); }
                if let Some(mut log) = log_file { writeln!(log, "\tWriting Service Bundle to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);

                let mut written = HashSet::new();
                for (type_id, metadata) in &service_metadata {
                    for (icon_kind, filename) in metadata {
                        if let Some(mut log) = log_file { writeln!(log, "\t\tType {} ({}) - {}", type_id, icon_kind, filename)?; }
                        if written.insert(filename) {
                            writer.start_file(filename, FileOptions::<()>::default().compression_method(CompressionMethod::Stored))
                                .map_err(|e| format!("err in {}: {}", filename, e))
                                .map_err(io::Error::other)?;
                            io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                        }
                    }
                }

                writer.start_file("service_metadata.json", FileOptions::<()>::default()).map_err(io::Error::other)?;
                serde_json::to_writer_pretty(&mut writer, &service_metadata).map_err(io::Error::other)?;

                writer.finish().map_err(io::Error::other)?.flush()?;
            }
            OutputMode::IEC { out } => {
                if skip_output {
                    if !silent_mode { println!("\tSKIPPED IEC archive"); }
                    if let Some(mut log) = log_file { writeln!(log, "\tSKIPPED IEC archive")?; }
                    continue;
                }

                if !silent_mode { println!("\tWriting IEC archive to {:?}", out); }
                if let Some(mut log) = log_file { writeln!(log, "\tWriting IEC archive to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);
                // Copy the icons IEC-style; Types with the same icon get duplicated files
                for (type_id, icons) in &service_metadata {
                    for (icon_kind, filename) in icons {
                        match icon_kind {
                            IconKind::Icon => {
                                let output_name = format!("{}_64.png", type_id);
                                if let Some(mut log) = log_file { writeln!(log, "\t\tType {} ({}) - {} [{}]", type_id, icon_kind, output_name, filename)?; }
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                            IconKind::Blueprint | IconKind::Reaction | IconKind::Relic => { /* None, these are duplicated by IconKind::Icon */ }
                            IconKind::BlueprintCopy => {
                                let output_name = format!("{}_bpc_64.png", type_id);
                                if let Some(mut log) = log_file { writeln!(log, "\t\tType {} ({}) - {} [{}]", type_id, icon_kind, output_name, filename)?; }
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                            IconKind::Render => {
                                let output_name = format!("{}_512.jpg", type_id);
                                if let Some(mut log) = log_file { writeln!(log, "\t\tType {} ({}) - {} [{}]", type_id, icon_kind, output_name, filename)?; }
                                writer.start_file(&output_name, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                                io::copy(&mut File::open(icon_dir.join(filename))?, &mut writer)?;
                            }
                        }
                    }
                }
                writer.finish().map_err(io::Error::other)?.flush()?;
            }
            OutputMode::Web { out, copy_files, hard_link } => {
                if skip_output {
                    if !silent_mode { println!("\tSKIPPED building web folder"); }
                    if let Some(mut log) = log_file { writeln!(log, "\tSKIPPED building web folder")?; }
                    continue;
                }

                let mode_name = if copy_files { "COPYING" } else if hard_link { "HARD LINK" } else { "SOFT LINK" };
                if !silent_mode { println!("\tBuilding web folder to {:?} ({})", out, mode_name); }
                if let Some(mut log) = log_file { writeln!(log, "\tBuilding web folder to {:?} ({})", out, mode_name)?; }
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
                    kind_buf.extend(icons.keys());
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
                            if let Some(mut log) = log_file { writeln!(log, "\t\t{} -> {}", &filename, &link_name)?; }
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
                            if let Some(mut log) = log_file { writeln!(log, "\t\tSKIP: {}", &link_name)?; }
                        }
                        created_files.insert(link_name, filename.clone());
                    }
                }

                for entry in old_links.keys() {
                    if !created_files.contains_key(entry) {
                        if let Some(mut log) = log_file { writeln!(log, "\t\tRemoved: {}", &entry)?; }
                        match fs::remove_file(out.join(entry)) {
                            Ok(()) => Ok(()),
                            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                            res => res
                        }?;
                    }
                }
                serde_json::to_writer(File::create(&index_path)?, &created_files).map_err(io::Error::other)?;
            }
            OutputMode::Checksum { out } => {
                // Checksum is never skipped

                let checksum = md5::compute(&index_bytes);
                if let Some(mut log) = log_file { writeln!(log, "Checksum:{:x}", checksum)?; }
                if let Some(outfile) = out {
                    if !silent_mode { println!("\tWriting checksum to {:?}", outfile); }
                    fs::write(outfile, format!("{:x}", checksum))?
                } else {
                    assert!(silent_mode);
                    print!("{:x}", md5::compute(&index_bytes))
                }
            },
            // Auxiliary outputs don't use the icon cache, but updating/checking it is quite fast so these outputs don't skip it
            OutputMode::AuxIcons { out } => {
                if skip_output {
                    if !silent_mode { println!("\tSKIPPED Auxiliary Icon dump archive"); }
                    if let Some(mut log) = log_file { writeln!(log, "\tSKIPPED Auxiliary Icon dump archive")?; }
                    continue;
                }

                if !silent_mode { println!("\tWriting Auxiliary Icon dump archive to {:?}", out); }
                if let Some(mut log) = log_file { writeln!(log, "\tWriting Auxiliary Icon dump archive to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);
                for (icon_id, resource) in &data.icon_files {
                    let (_path, extension) = resource.rsplit_once('.')
                        .or_else(|| resource.rsplit_once('/'))
                        .unwrap_or(("", resource));

                    if !silent_mode { println!("\t\t{}: {}", icon_id, resource); };
                    if let Some(mut log) = log_file { writeln!(log, "\t\t{}: {}", icon_id, resource)?; }

                    let resource_path = cache.path_of(resource)?;
                    writer.start_file(format!("{}.{}", icon_id, extension), FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                    std::io::copy(&mut File::open(resource_path)?, &mut writer)?;
                }
                writer.finish().map_err(io::Error::other)?.flush()?;
            }
            OutputMode::AuxImages { out, incl_character } => {
                if skip_output {
                    if !silent_mode { println!("\tSKIPPED Auxiliary All-Images dump archive"); }
                    if let Some(mut log) = log_file { writeln!(log, "\tSKIPPED Auxiliary All-Images dump archive")?; }
                    continue;
                }

                if !silent_mode { println!("\tWriting Auxiliary All-Images dump archive to {:?}", out); }
                if let Some(mut log) = log_file { writeln!(log, "\tWriting Auxiliary All-Images dump archive to {:?}", out)?; }
                let mut writer = ZipWriter::new(File::create(out)?);

                let resource_valid = |resource: &&str| (resource.ends_with("png") || resource.ends_with("jpg")) && (incl_character || !resource.starts_with("res:/graphics/character/"));

                let res_count = cache.iter_resources().filter(resource_valid).count();
                for (n, resource) in cache.iter_resources().filter(resource_valid).enumerate() {
                    let (_resource_kind, filename) = resource.split_once(":/").unwrap_or(("", resource));
                    let resource_path = cache.path_of(resource)?;

                    if !silent_mode { println!("\t\t[{}/{}] {}", n, res_count, resource); }
                    if let Some(mut log) = log_file { writeln!(log, "\t\t[{}/{}] {}", n, res_count, resource)?; }

                    writer.start_file(filename, FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).map_err(io::Error::other)?;
                    std::io::copy(&mut File::open(resource_path)?, &mut writer)?;
                }
                writer.finish().map_err(io::Error::other)?.flush()?;
            },
        }
    }

    if !no_purge {
        for filename in &to_remove {
            fs::remove_file(icon_dir.join(filename))?;
        }
    }

    Ok((to_add.len(), to_remove.len()))
}
