//! CAVEAT EMPTOR
//!
//! These are manually put together data-lists, to which the following caveats apply:
//! * Updates are manual and users must download the latest version of this crate to receive them.
//! * Data is likely to be outdated.
//! * Data may be erroneous.
//! * Data ignores CCP data marked as "un-published" unless explicitly stated otherwise.
//!
//! We are not responsible if your space pixels explode.

use std::io::Write;

#[cfg(feature = "serde")]
pub fn export<W: Write>(out: W) {
    let holds = [
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
    ];

    use indexmap::IndexMap;
    serde_json::to_writer_pretty(out, &IndexMap::from(holds)).unwrap(); // Indexmap to retain order
}

pub mod cargo {
    use crate::item_list::TypeList;
    use crate::ids::AttributeID;

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
