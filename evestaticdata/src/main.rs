use evestaticdata::sde::update::SdeVersion;
use std::error::Error;
use std::{fs};
use std::fs::File;
use evestaticdata::sde::load::SDELoader;

pub fn main() -> Result<(), Box<dyn Error>> {
    // let version = sde::update::update_sde("./temp/sde.zip")?;
    // println!("{}", version);
    SDELoader::new(File::open("./temp/eve-online-static-data-jsonl.zip")?)?.full()?;

    return Ok(());
    let mut version = SdeVersion::fetch_latest()?;
    println!("Latest: {}", version.build_number());

    version.download_sde("./temp/diff/latest.zip")?;
    let mut latest_build_number = version.build_number();

    while version.build_number() > 2960198 {
        version = version.previous()?;
        let prev_build_number = version.build_number();
        version.download_sde("./temp/diff/previous.zip")?;

        let out_path = format!("./temp/diff/out/{}→{}.zip", prev_build_number, latest_build_number);
        if fs::exists(&out_path)? {
            println!("\tDiff already exists: {}", out_path);
            break;
        }
        println!("\tBuilding: {}", out_path);
        evestaticdata::sde::diff::build_diff("./temp/diff/latest.zip", "./temp/diff/previous.zip", out_path)?;

        fs::rename("./temp/diff/previous.zip", "./temp/diff/latest.zip")?;
        latest_build_number = prev_build_number;
    }


    Ok(())
}
