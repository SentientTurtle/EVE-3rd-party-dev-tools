#![feature(iter_array_chunks)]

pub mod ids;
pub mod numbers;
pub mod item_list;
pub mod units;
pub mod sde;
pub mod hardcoded;

#[cfg(test)]
pub mod test {    
    pub mod sde_update {
        use std::error::Error;
        use crate::sde::ccp_sde::update::SDEChecksums;

        /// Ensure default debug does not panic
        #[test]
        fn checksum_default_debug() {
            let _ = format!("{:?}", SDEChecksums::default());
        }
        
        /// Download checksums
        #[test]
        #[ignore = "Touches remote servers and should be ran manually"]
        fn checksum_download() -> Result<(), Box<dyn Error>> {
            let checksums = SDEChecksums::download()?;
            
            let nul_string = std::str::from_utf8(&[0; 32]).unwrap();
            assert_ne!(checksums.sde(), nul_string);
            assert_ne!(checksums.fsd(), nul_string);
            assert_ne!(checksums.bsd(), nul_string);
            assert_ne!(checksums.universe(), nul_string);
            
            Ok(())
        }
    }

    #[cfg(feature = "load_yaml")]
    pub mod sde_load {
        use std::fs::File;
        use zip::ZipArchive;
        use crate::sde;
        use crate::sde::ccp_sde::update::SDEKind;

        #[test]
        #[ignore = "Touches remote servers and should be ran manually"]
        fn test_bsd_load() {
            let (sde_path, _) = SDEKind::BSD.update("./temp").unwrap();
            let mut archive = ZipArchive::new(File::open(sde_path).unwrap()).unwrap();
            let _bsd = sde::ccp_sde::load::do_load_bsd(&mut archive).unwrap();
        }

        #[test]
        #[ignore = "Touches remote servers and should be ran manually"]
        fn test_fsd_load() {
            let (sde_path, _) = SDEKind::FSD.update("./temp").unwrap();
            let mut archive = ZipArchive::new(File::open(sde_path).unwrap()).unwrap();
            let _fsd = sde::ccp_sde::load::do_load_fsd(&mut archive).unwrap();
        }

        #[test]
        #[ignore = "Touches remote servers and should be ran manually"]
        fn test_universe_load() {
            let (sde_path, _) = SDEKind::UNIVERSE.update("./temp").unwrap();
            let mut archive = ZipArchive::new(File::open(sde_path).unwrap()).unwrap();
            let _fsd = sde::ccp_sde::load::do_load_universe(&mut archive).unwrap();
        }
    }
}