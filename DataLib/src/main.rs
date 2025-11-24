use std::error::Error;
use std::fs::File;
use std::time::Instant;
use zip::ZipArchive;
use evestaticdata::sde::load::load_all;

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("{}\n{}\n{}", evestaticdata::CRATE_NAME, evestaticdata::CRATE_VERSION, evestaticdata::CRATE_REPO);

    let start = Instant::now();
    evestaticdata::sde::update::update_sde("./temp/sde.zip")?;

    let _sde = load_all(&mut ZipArchive::new(File::open("./temp/sde.zip")?)?)?;
    let load_time = start.elapsed().as_millis();

    println!("Loaded in: {}ms", load_time);

    Ok(())
}