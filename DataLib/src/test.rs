#[cfg(test)]
#[cfg(feature = "load_yaml")]
pub mod sde_load {

    #[cfg(test)]
    pub mod sde {
        use std::fs::File;
        use std::io;
        use zip::ZipArchive;
        use crate::sde;

        #[test]
        #[ignore = "Touches remote servers and should be ran manually"]
        fn test_sde_load() -> Result<(), io::Error> {
            let sde_path = "./temp/sde.zip";
            sde::ccp_sde::update::update_sde(sde_path)?;

            let _sde = sde::ccp_sde::load::load_all(&mut ZipArchive::new(File::open(sde_path)?).map_err(io::Error::other)?).unwrap();
            Ok(())
        }
    }
}
