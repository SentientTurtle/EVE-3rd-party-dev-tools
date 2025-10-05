use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::Instant;
use zip::ZipArchive;
use evestaticdata::sde::load::load_all;

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("{}\n{}\n{}", evestaticdata::CRATE_NAME, evestaticdata::CRATE_VERSION, evestaticdata::CRATE_REPO);

    let start = Instant::now();
    let _sde = load_all(&mut ZipArchive::new(File::open("./temp/sde.zip")?)?)?;
    println!("Loaded in: {}ms", start.elapsed().as_millis());

    let mut outfile = File::create("./effects.txt")?;

    let mut archive = ZipArchive::new(File::open("./temp/sde_old.zip")?)?;
    for i in 0..archive.len() {
        let name = archive.name_for_index(i).unwrap_or("");
        if name.ends_with("solarsystem.yaml") {
            let mut solar_system_id = Option::<u32>::None;
            let mut effect_beacon = Option::<u32>::None;
            for line in BufReader::new(archive.by_index(i)?).lines() {
                let line = line?;
                if let Some(id) = line.strip_prefix("solarSystemID: ") {
                    solar_system_id = Some(u32::from_str(id)?);
                }
                if let Some(effect) = line.strip_prefix("  effectBeaconTypeID: ") {
                    effect_beacon = Some(u32::from_str(effect)?);
                }
            }
            use std::io::Write;
            if let (Some(id), Some(effect)) = (solar_system_id, effect_beacon) {
                writeln!(outfile, "({}, {}),", id, effect)?;
            }
        }
    }


    Ok(())
}