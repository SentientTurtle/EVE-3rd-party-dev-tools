//! CAVEAT EMPTOR
//!
//! These are manually put together data-lists, to which the following caveats apply:
//! * Updates are manual and users must download the latest version of this crate to receive them.
//! * Data is likely to be outdated.
//! * Data may be erroneous.
//! * Data ignores CCP data marked as "un-published" unless explicitly stated otherwise.
//!
//! We are not responsible if your space pixels explode.

#[cfg(feature = "serde")]
pub fn export<W: std::io::Write>(out: W) {
    // Indexmap to retain order
    let constants = IndexMap::from([
        ("MAX_TARGETING_RANGE", magic_constants::MAX_TARGETING_RANGE)
    ]);
    
    let holds = IndexMap::from([
        ("SMB", cargo::SHIP_MAINTENANCE_BAY),
        ("SMB_RORQ", cargo::SHIP_MAINTENANCE_BAY_RORQUAL),
        ("FLEET", cargo::FLEET_HANGAR),
        ("FUEL", cargo::FUEL_BAY),
        ("MINING", cargo::MINING_HOLD),
        ("GAS", cargo::GAS_HOLD),
        ("MINERAL", cargo::MINERAL_HOLD),
        ("AMMO", cargo::AMMO_HOLD),
        ("COMMAND_CENTER", cargo::COMMAND_CENTER_HOLD),
        ("PI", cargo::PLANETARY_COMMODITIES_HOLD),
        ("QUAFE", cargo::QUAFE_HOLD),
        ("CORPSE", cargo::CORPSE_HOLD),
        ("BOOSTER", cargo::BOOSTER_HOLD),
        ("SUBSYSTEM", cargo::SUBSYSTEM_HOLD),
        ("ICE", cargo::ICE_HOLD),
        ("DEPOT", cargo::MOBILE_DEPOT_HOLD),
        ("INFRASTRUCTURE", cargo::INFRASTRUCTURE_HOLD),
    ]);
    
    #[derive(serde::Serialize)]
    struct Exports {
        constants: IndexMap<&'static str, f64>,
        holds: IndexMap<&'static str, cargo::CargoHoldType<'static>>
    }

    use indexmap::IndexMap;
    serde_json::to_writer_pretty(out, &Exports { constants, holds }).unwrap();
}

pub mod magic_constants {
    pub const MAX_TARGETING_RANGE: f64 = 300_000.0;
}

pub mod id_ranges {
    use std::ops::RangeInclusive;

    pub const VARIOUS: RangeInclusive<u32> = 0..=499_999;
    pub const FACTIONS: RangeInclusive<u32> = 500_000..=599_999;
    pub const NPC_CORPS: RangeInclusive<u32> = 1_000_000..=1_999_999;
    pub const NPC_CHARS: RangeInclusive<u32> = 3_000_000..=3_999_999;
    pub const UNIVERSES: RangeInclusive<u32> = 9_000_000..=9_999_999;
    pub const REGIONS: RangeInclusive<u32> = 10_000_000..=19_999_999;
    pub const CONSTELLATIONS: RangeInclusive<u32> = 20_000_000..=29_999_999;
    pub const SOLARSYSTEMS: RangeInclusive<u32> = 30_000_000..=39_999_999;
    pub const CELESTIALS: RangeInclusive<u32> = 40_000_000..=49_999_999;
    pub const STARGATES: RangeInclusive<u32> = 50_000_000..=59_999_999;
    pub const STATIONS: RangeInclusive<u32> = 60_000_000..=69_999_999;
    pub const ASTEROIDS: RangeInclusive<u32> = 70_000_000..=79_999_999; // Note: *NOT* Asteroid Belts, ids::AsteroidBeltID is under CELESTIALS
    pub const CONTROL_BUNKERS: RangeInclusive<u32> = 80_000_000..=80_099_999;
    pub const WIS_PROMENADES: RangeInclusive<u32> = 81_000_000..=81_999_999;    // Press 'F' to pay respects
    pub const PLANETARY_DISTRICTS: RangeInclusive<u32> = 82_000_000..=84_999_999;
    pub const EVE_CHARS_2: RangeInclusive<u32> = 90_000_000..=97_999_999;
    pub const EVE_CORPS_2: RangeInclusive<u32> = 98_000_000..=98_999_999;
    pub const EVE_ALLIANCES_2: RangeInclusive<u32> = 99_000_000..=99_999_999;
    pub const EVE_MIXED_CHARS_CORPS_ALLIANCES_1: RangeInclusive<u32> = 100_000_000..=2_099_999_999;
    pub const EVE_CHARS_3: RangeInclusive<u32> = 2_100_000_000..=2_111_999_999;
    pub const EVE_CHARS_4: RangeInclusive<u32> = 2_112_000_000..=2_129_999_999;
}

pub mod cargo {
    use crate::util::item_list::TypeList;
    use crate::types::ids::AttributeID;

    #[cfg_attr(feature = "serde", derive(serde::Serialize))]
    pub struct CargoHoldType<'a> {
        pub attribute_id: Option<AttributeID>,
        pub filter: Option<TypeList<'a>>,
        pub packaged_ships: bool,
        pub assembled_ships: bool,
    }

    pub const SHIP_MAINTENANCE_BAY: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(908),
        filter: Some(TypeList {
            included_categories: &[6],  // Ships
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: true,
    };

    // TODO: Validate with attribute 1891
    pub const SHIP_MAINTENANCE_BAY_RORQUAL: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(908),
        filter: Some(TypeList { // TODO: Verify this list
            included_groups: &[
                28,     // Hauler
                380,    // Deep Space Transport
                1202,   // Blockade Runner
                463,    // Mining Barge
                543,    // Exhumer
            ],
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: true,
    };

    pub const FLEET_HANGAR: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(912),
        filter: None,
        packaged_ships: true,
        assembled_ships: true,
    };

    pub const FUEL_BAY: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1549),
        filter: Some(TypeList {
            included_groups: &[423],    // Ice product
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const MINING_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1556),
        filter: Some(TypeList { // TODO: Verify this list
            included_groups: &[711],    // Gas cloud
            included_categories: &[25], // Asteroid (= Ore types)
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const GAS_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1557),
        filter: Some(TypeList {
            included_groups: &[711],    // Gas cloud
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const MINERAL_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1558),
        filter: Some(TypeList {
            included_groups: &[18],    // Mineral
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
    
    pub const AMMO_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1573),
        filter: Some(TypeList {
            included_categories: &[8],    // Charge
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
    
    pub const COMMAND_CENTER_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1646),
        filter: Some(TypeList {
            included_groups: &[1027],   // Command Center
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
    
    pub const PLANETARY_COMMODITIES_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1653),
        filter: Some(TypeList {
            included_categories: &[
                42,     // Planetary Resources (T0/Raw resources)
                43      // Planetary Commodities
            ],
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
    
    // TODO: Possibly remove as the Quafe-edition ships with this have been converted into a SKIN?
    pub const QUAFE_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(1804),
        filter: Some(TypeList {
            included_types: &[
                3699,
                12865,
                57422,
                21661,
                3898,
                60575,
                12994,
            ],
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
    
    pub const CORPSE_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(2467),
        filter: Some(TypeList {
            included_groups: &[14], // Biomass (corpses)
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const BOOSTER_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(2657),
        filter: Some(TypeList {
            included_groups: &[303], // Booster
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const SUBSYSTEM_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(2675),
        filter: Some(TypeList {
            included_categories: &[32], // Subsystem
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const ICE_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(3136),
        filter: Some(TypeList {
            included_groups: &[465], // Ice
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const MOBILE_DEPOT_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(5325),
        filter: Some(TypeList {
            included_groups: &[1246], // Mobile Depot
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };

    pub const INFRASTRUCTURE_HOLD: CargoHoldType<'static> = CargoHoldType {
        attribute_id: Some(5646),
        filter: Some(TypeList { // TODO Verify this list, in particular: PI control centers
            included_categories: &[
                42,     // Planetary Resources (T0/Raw resources)
                43,     // Planetary Commodities
                65,     // (Upwell) Structure
                66,     // Structure Module
                40,     // Sovereignty Structures (TODO (low priority): This category includes TCUs, verify if those are allowed)
                39,     // Infrastructure Upgrades
                22,     // Deployable
            ],
            included_groups: &[
                4729,   // Colony Reagents
                1546,   // Structure Anti-Capital Missile
                1547,   // Structure Anti-Subcapital Missile
                1548,   // (Structure) Guided Bomb
                1549,   // Structure ECM script
                1551,   // Structure Warp Disruptor Script
                1976,   // Structure Festival Charges
                4186,   // Structure Area Denial Ammunition
                4777,   // Structure Light Fighter
                4778,   // Structure Support Fighter
                4779,   // Structure Heavy Fighter
                4736,   // Skyhook
                1106,   // Orbital Construction Platform (Custom's Gantry)
                427,    // Moon Materials
                1136,   // Fuel Block
                42,     // Planetary Resources (T0/Raw resources)
                43,     // Planetary Commodities
                423,    // Ice product
            ],
            ..TypeList::empty()
        }),
        packaged_ships: false,
        assembled_ships: false,
    };
}

pub mod wormhole {
    use crate::types::ids;

    pub enum WormholeEffect {
        PulsarC1,
        PulsarC2,
        PulsarC3,
        PulsarC4,
        PulsarC5,
        PulsarC6,

        BlackHoleC1,
        BlackHoleC2,
        BlackHoleC3,
        BlackHoleC4,
        BlackHoleC5,
        BlackHoleC6,

        CataclysmicVariableC1,
        CataclysmicVariableC2,
        CataclysmicVariableC3,
        CataclysmicVariableC4,
        CataclysmicVariableC5,
        CataclysmicVariableC6,

        MagnetarC1,
        MagnetarC2,
        MagnetarC3,
        MagnetarC4,
        MagnetarC5,
        MagnetarC6,

        RedGiantC1,
        RedGiantC2,
        RedGiantC3,
        RedGiantC4,
        RedGiantC5,
        RedGiantC6,

        WolfRayetC1,
        WolfRayetC2,
        WolfRayetC3,
        WolfRayetC4,
        WolfRayetC5,
        WolfRayetC6,

        WolfRayetC13,
    }

    impl WormholeEffect {
        pub fn beacon_id(&self) -> ids::TypeID {
            match self {
                WormholeEffect::PulsarC1 => 30844,
                WormholeEffect::PulsarC2 => 30865,
                WormholeEffect::PulsarC3 => 30866,
                WormholeEffect::PulsarC4 => 30867,
                WormholeEffect::PulsarC5 => 30868,
                WormholeEffect::PulsarC6 => 30869,
                WormholeEffect::BlackHoleC1 => 30845,
                WormholeEffect::BlackHoleC2 => 30850,
                WormholeEffect::BlackHoleC3 => 30851,
                WormholeEffect::BlackHoleC4 => 30852,
                WormholeEffect::BlackHoleC5 => 30853,
                WormholeEffect::BlackHoleC6 => 30854,
                WormholeEffect::CataclysmicVariableC1 => 30846,
                WormholeEffect::CataclysmicVariableC2 => 30880,
                WormholeEffect::CataclysmicVariableC3 => 30881,
                WormholeEffect::CataclysmicVariableC4 => 30884,
                WormholeEffect::CataclysmicVariableC5 => 30883,
                WormholeEffect::CataclysmicVariableC6 => 30882,
                WormholeEffect::MagnetarC1 => 30847,
                WormholeEffect::MagnetarC2 => 30860,
                WormholeEffect::MagnetarC3 => 30861,
                WormholeEffect::MagnetarC4 => 30862,
                WormholeEffect::MagnetarC5 => 30863,
                WormholeEffect::MagnetarC6 => 30864,
                WormholeEffect::RedGiantC1 => 30848,
                WormholeEffect::RedGiantC2 => 30870,
                WormholeEffect::RedGiantC3 => 30871,
                WormholeEffect::RedGiantC4 => 30872,
                WormholeEffect::RedGiantC5 => 30873,
                WormholeEffect::RedGiantC6 => 30874,
                WormholeEffect::WolfRayetC1 => 30849,
                WormholeEffect::WolfRayetC2 => 30875,
                WormholeEffect::WolfRayetC3 => 30876,
                WormholeEffect::WolfRayetC4 => 30877,
                WormholeEffect::WolfRayetC5 => 30878,
                WormholeEffect::WolfRayetC6 => 30879,
                WormholeEffect::WolfRayetC13 => 30879,  // Uses the C6 effect beacon
            }
        }
    }

    // `WORMHOLE_EFFECTS` is removed; Data back in the SDE
}