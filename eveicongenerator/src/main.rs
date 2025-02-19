#![feature(exit_status_error)]
#![feature(iter_intersperse)]
#![feature(path_add_extension)]
#![feature(iter_collect_into)]

use crate::icons::{IconBuildData, IconError, OutputMode};
use crate::sde::update_sde;
use evesharedcache::cache::CacheDownloader;
use std::time::Instant;
use std::{fs, io};
use std::path::PathBuf;
use clap::{Arg, ArgAction, Command};
use clap::builder::ValueParser;

pub mod icons;
pub mod sde;

fn main() {
    match do_main() {
        Ok(()) => {}
        Err(err) => println!("Error: {}", err)
    }
}

fn do_main() -> Result<(), IconError> {
    let arg_matches = Command::new("eveicongenerator")
        .about("Multi-purpose item-icon generator for EVE Online")
        .args([
            Arg::new("cache_folder")
                .short('c')
                .long("cache_folder")
                .help("Game data cache folder to use")
                .default_value("./cache")
                .value_parser(ValueParser::path_buf()),
            Arg::new("icon_folder")
                .short('i')
                .long("icon_folder")
                .help("Output/Cache folder for icons")
                .default_value("./icons")
                .value_parser(ValueParser::path_buf()),
            Arg::new("data")
                .short('d')
                .long("data")
                .value_parser(["SDE", #[cfg(feature="enable_fsd")] "FSD"])
                .requires_if("FSD", "python2")
                .default_value("SDE")
                .help("Data source to use"),
            Arg::new("python2")
                .long("python2")
                .required_if_eq("data", "FSD")
                .help("command to python 2.7, required for FSD use")
                .value_parser(ValueParser::string()),
            Arg::new("force_rebuild")
                .short('f')
                .long("force_rebuild")
                .help("Force-rebuild of unchanged icons")
                .action(ArgAction::SetTrue),
            Arg::new("skip_if_fresh")
                .short('s')
                .long("skip_if_fresh")
                .help("If icons are unchanged, skip output")
                .action(ArgAction::SetTrue),
            Arg::new("use_magick")
                .long("use_magick")
                .help("Use imagemagick 7 for image compositing")
                .action(ArgAction::SetTrue)
        ])
        .subcommand_required(true)
        .subcommands([
            Command::new("service_bundle")
                .about("Image Service hosting bundle (zip incl. metadata)")
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .required(true)
                        .help("Output file")
                        .value_parser(ValueParser::path_buf())
                ),
            Command::new("iec")
                .about("Image Export Collection (zip)")
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .required(true)
                        .help("Output file")
                        .value_parser(ValueParser::path_buf())
                ),
            Command::new("web_dir")
                .about("Prepare a directory for web hosting")
                .args([
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .required(true)
                        .help("Output directory")
                        .value_parser(ValueParser::path_buf()),
                    Arg::new("copy_files")
                        .long("copy_files")
                        .help("Copy image files rather than creating symlinks")
                        .action(ArgAction::SetTrue),
                    Arg::new("hardlink")
                        .long("hardlink")
                        .help("Use hard-links rather than soft-links")
                        .action(ArgAction::SetTrue)
                ])
        ])
        .get_matches();

    let (command_name, command_args) = arg_matches.subcommand().expect("subcommand required");
    let output_mode = match command_name {
        "service_bundle" => OutputMode::ServiceBundle { out: &command_args.get_one::<PathBuf>("out").expect("out is required") },
        "iec" => OutputMode::IEC { out: &command_args.get_one::<PathBuf>("out").expect("out is required") },
        "web_dir" => {
            let out = &command_args.get_one::<PathBuf>("out").expect("out is required");
            if !fs::exists(out)? {
                fs::create_dir_all(out)?;
            } else if fs::metadata(out)?.is_file() {
                Err(io::Error::other(format!("Output must be a directory! ({})", out.to_string_lossy())))?;
            }
            OutputMode::Web {
                out,
                copy_files: command_args.get_flag("copy_files"),
                hard_link: command_args.get_flag("hardlink")
            }
        },
        _ => panic!("Unknown subcommand: {}", command_name)
    };


    let start = Instant::now();

    println!("Initializing cache");
    let cache = CacheDownloader::initialize(arg_matches.get_one::<PathBuf>("cache_folder").expect("cache_folder is a required argument"), false).unwrap();
    let cache_init_duration = start.elapsed();

    let data_load_start = Instant::now();
    let data_source = arg_matches.get_one::<String>("data").expect("Data arg must always be present as it has a default-value");
    let icon_build_data = match data_source.as_str() {
        "SDE" => {
            println!("Loading SDE...");
            let mut sde = update_sde()?;

            IconBuildData::new(
                sde::read_types(&mut sde)?.into_iter().collect(),
                sde::read_group_categories(&mut sde)?,
                sde::read_icons(&mut sde)?,
                sde::read_graphics(&mut sde)?,
                sde::read_skin_materials(&mut sde)?
            )
        },
        #[cfg(feature="enable_fsd")]
        "FSD" => {
            use crate::icons::{TypeInfo};
            use std::fs;
            use std::collections::HashMap;

            println!("Loading python FSD...");
            let python2 = arg_matches.get_one::<String>("python2").expect("python2 must be present in FSD mode!");
            let temp_dir = "./fsd";
            fs::create_dir_all(temp_dir)?;
            IconBuildData::new(
                evesharedcache::fsd::read_types(&cache, python2, temp_dir).map_err(io::Error::other)?
                    .into_iter()
                    .map(|(type_id, eve_type)| (type_id, TypeInfo { group_id: eve_type.groupID, icon_id: eve_type.iconID, graphic_id: eve_type.graphicID, meta_group_id: eve_type.metaGroupID, }))
                    .collect(),
                evesharedcache::fsd::read_groups(&cache, python2, temp_dir).map_err(io::Error::other)?
                    .into_iter()
                    .map(|(group_id, group)| (group_id, group.categoryID))
                    .collect(),
                evesharedcache::fsd::read_icons(&cache, python2, temp_dir).map_err(io::Error::other)?
                    .into_iter()
                    .map(|(icon_id, icon)| (icon_id, icon.iconFile))
                    .collect(),
                evesharedcache::fsd::read_graphics(&cache, python2, temp_dir).map_err(io::Error::other)?
                    .into_iter()
                    .filter_map(|(graphic_id, graphic)| graphic.iconInfo.map(|icon_info| (graphic_id, icon_info.folder)))
                    .collect(),
                {
                    let skin_licenses = evesharedcache::static_sqlite::load_skin_licenses(&cache).map_err(io::Error::other)?;
                    let skins = evesharedcache::static_sqlite::load_skins(&cache).map_err(io::Error::other)?;

                    let mut skin_license_materials = HashMap::<u32, u32>::new();

                    for (license_id, license) in skin_licenses {
                        // Some unused licenses exist in the data, but their associated skins do not exist
                        if let Some(skin) = skins.get(&license.skinID) {
                            skin_license_materials.insert(license_id, skin.skinMaterialID);
                        }
                    }

                    skin_license_materials
                }
            )
        },
        _ => unreachable!("Invalid value on data")
    };

    let data_load_duration = data_load_start.elapsed();

    println!("Building icons...");
    let build_start = Instant::now();
    let (added, removed) = icons::build_icon_export(
        output_mode,
        arg_matches.get_flag("skip_if_fresh"),
        &icon_build_data,
        &cache,
        arg_matches.get_one::<PathBuf>("icon_folder").expect("icon_folder is a required argument"),
        arg_matches.get_flag("force_rebuild"),
        arg_matches.get_flag("use_magick")
    )?;

    let build_duration = build_start.elapsed();

    println!("Finished in: {:.1} seconds", start.elapsed().as_secs_f64());
    println!("\tCache init: {:.1} seconds", cache_init_duration.as_secs_f64());
    println!("\tData load: {:.1} seconds ({})", data_load_duration.as_secs_f64(), data_source);
    println!("\tImage Build: {:.1} seconds ({} added, {} removed)", build_duration.as_secs_f64(), added, removed);

    // Delete unnecessary cache files to avoid a storage "leak"
    cache.purge(&["fsd.zip", "checksum.txt"])?;

    Ok(())
}
