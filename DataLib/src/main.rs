use std::error::Error;
use std::fs::File;
use std::time::Instant;
use evestaticdata::sde;
use evestaticdata::sde::load::SDELoader;

pub fn main() -> Result<(), Box<dyn Error>> {

    sde::update::update_sde("./temp/sde.zip")?;

    let start = Instant::now();
    let sde = SDELoader::new(File::open("./temp/sde.zip")?)?.full()?;
    println!("Loaded in: {}s", start.elapsed().as_secs_f64());

    Ok(())
}