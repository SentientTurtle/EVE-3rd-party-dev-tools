#![feature(exit_status_error)]
#![feature(iter_intersperse)]
#![feature(path_add_extension)]
#![feature(iter_collect_into)]

use crate::icons::{IconBuildData, IconError, OutputMode};
use crate::sde::update_sde;
use evesharedcache::cache::CacheDownloader;
use std::time::Instant;
use std::{fs, io};
use std::fs::File;
use std::path::PathBuf;
use std::sync::OnceLock;
use clap::{Arg, ArgAction, Command};
use clap::builder::ValueParser;
use std::io::Write;

pub mod icons;
pub mod sde;

static LOG_FILE: OnceLock<File> = OnceLock::new();

fn main() {
    match do_main() {
        Ok(()) => {}
        Err(err) => {
            println!("Error: {}", err);
            let log_file = LOG_FILE.get();
            if let Some(mut log) = log_file {
                writeln!(log, "Error: {}", err).unwrap();
            }
        }
    }
}

fn do_main() -> Result<(), IconError> {
    let arg_matches = Command::new("eveicongenerator")
        .about("Multi-purpose item-icon generator for EVE Online")
        .args([
            Arg::new("user_agent")
                .short('u')
                .long("user_agent")
                .help("User Agent for HTTP requests")
                .required(true)
                .value_parser(ValueParser::string()),
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
            Arg::new("logfile")
                .short('l')
                .long("logfile")
                .help("Log file to use, no logging if unset")
                .value_parser(ValueParser::path_buf()),
            Arg::new("append_log")
                .long("append_log")
                .help("Append to log file, if unset replaces log file")
                .requires("logfile")
                .action(ArgAction::SetTrue),
            Arg::new("silent")
                .long("silent")
                .help("Silent mode")
                .action(ArgAction::SetTrue),
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
                        .conflicts_with("copy_files")
                        .action(ArgAction::SetTrue)
                ]),
            Command::new("checksum")
                .about("Prints (or writes) the checksum of the current icon set")
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .help("Output file, if omitted, prints checksum to stdout")
                        .value_parser(ValueParser::path_buf())
                ),
            Command::new("aux_icons")
                .about("Auxiliary Icon dump (zip)")
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .required(true)
                        .help("Output file")
                        .value_parser(ValueParser::path_buf())
                ),
            Command::new("aux_all")
                .about("Auxiliary All-Images dump (zip)")
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .required(true)
                        .help("Output file")
                        .value_parser(ValueParser::path_buf())
                ),
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
        "checksum" => OutputMode::Checksum { out: command_args.get_one::<PathBuf>("out").map(PathBuf::as_path) },
        "aux_icon" => OutputMode::AuxIcons { out: &command_args.get_one::<PathBuf>("out").expect("out is required") },
        "aux_all" => OutputMode::AuxImages { out: &command_args.get_one::<PathBuf>("out").expect("out is required") },
        _ => panic!("Unknown subcommand: {}", command_name)
    };

    let silent_mode = arg_matches.get_flag("silent") || matches!(output_mode, OutputMode::Checksum { out: None });
    let skip_if_fresh = arg_matches.get_flag("skip_if_fresh") && !matches!(output_mode, OutputMode::Checksum { out: None });

    if let Some(log_path) = arg_matches.get_one::<PathBuf>("logfile") {
        let mut opts = File::options();
        if arg_matches.get_flag("append_log") {
            opts.create(true).append(true);
        } else {
            opts.create(true).write(true).truncate(true);
        }

        LOG_FILE.set(opts.open(log_path)?).expect("Log file is set only once!");
    }
    let log_file = LOG_FILE.get();
    if let Some(mut log) = log_file { writeln!(log, "Icon generation run, output: {:?} - {}", &output_mode, chrono::Local::now())?; }

    let user_agent = arg_matches.get_one::<String>("user_agent").expect("user_agent is a required argument");

    let start = Instant::now();
    if !silent_mode { println!("Initializing cache (UA:`{}`)", user_agent); }
    if let Some(mut log) = log_file { writeln!(log, "Initializing cache (UA:`{}`)", user_agent)?; }
    let cache = CacheDownloader::initialize(
        arg_matches.get_one::<PathBuf>("cache_folder").expect("cache_folder is a required argument"),
        false,
        user_agent
    )?;
    let cache_init_duration = start.elapsed();

    let data_load_start = Instant::now();
    let icon_build_data = {
        if !silent_mode { println!("Loading SDE..."); }
        if let Some(mut log) = log_file { writeln!(log, "Loading SDE...")?; }
        let mut sde = update_sde(silent_mode)?;

        IconBuildData::new(
            sde::read_types(&mut sde, silent_mode)?.into_iter().collect(),
            sde::read_group_categories(&mut sde, silent_mode)?,
            sde::read_icons(&mut sde, silent_mode)?,
            sde::read_graphics(&mut sde, silent_mode)?,
            sde::read_skin_materials(&mut sde, silent_mode)?
        )
    };

    let data_load_duration = data_load_start.elapsed();

    if !silent_mode { println!("Building icons..."); }
    if let Some(mut log) = log_file { writeln!(log, "Building icons...")?; }

    let build_start = Instant::now();
    let (added, removed) = icons::build_icon_export(
        output_mode,
        skip_if_fresh,
        &icon_build_data,
        &cache,
        arg_matches.get_one::<PathBuf>("icon_folder").expect("icon_folder is a required argument"),
        arg_matches.get_flag("force_rebuild"),
        arg_matches.get_flag("use_magick"),
        silent_mode
    )?;

    let build_duration = build_start.elapsed();

    let s1 = format!("Finished in: {:.1} seconds", start.elapsed().as_secs_f64());
    let s2 = format!("\tCache init: {:.1} seconds", cache_init_duration.as_secs_f64());
    let s3 = format!("\tData load: {:.1} seconds", data_load_duration.as_secs_f64());
    let s4 = format!("\tImage Build: {:.1} seconds ({} added, {} removed)", build_duration.as_secs_f64(), added, removed);

    if !silent_mode {
        println!("{}", s1);
        println!("{}", s2);
        println!("{}", s3);
        println!("{}", s4);
    }
    if let Some(mut log) = log_file {
        writeln!(log, "{}", s1)?;
        writeln!(log, "{}", s2)?;
        writeln!(log, "{}", s3)?;
        writeln!(log, "{}", s4)?;
    }

    // Delete unnecessary cache files to avoid a storage "leak"
    cache.purge(&["sde.zip", "checksum.txt"])?;

    Ok(())
}
