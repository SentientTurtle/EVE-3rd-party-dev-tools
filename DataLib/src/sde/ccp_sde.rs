#[cfg(feature = "load_yaml")]
pub mod load {
    use std::collections::HashMap;
    use std::io::{Read, Seek};
    use serde::{Deserialize, Deserializer};
    use zip::read::ZipFile;
    use zip::result::ZipError;
    use zip::ZipArchive;
    use crate::{ids, numbers};
    use crate::units::EVEUnit;

    #[derive(Debug)]
    pub enum SDELoadError {
        /// An error occurred parsing the .zip file; Archive corrupt?
        MalformedZip(ZipError),
        /// SDE zip file did not contain expected file; Attempt to parse subset-package as full SDE?
        ArchiveFileNotFound(String),
        ParseError { file: String, error: serde_yaml_ng::Error},
        MalformedSDE,
    }

    impl From<ZipError> for SDELoadError {
        fn from(value: ZipError) -> Self {
            SDELoadError::MalformedZip(value)
        }
    }

    fn load_file<T, R: Read + Seek>(archive: &mut ZipArchive<R>, file_name: &str, loader: fn(ZipFile<R>) -> Result<T, serde_yaml_ng::Error>) -> Result<T, SDELoadError> {
        match archive.by_name(file_name) {
            Ok(file) => loader(file).map_err(|error| SDELoadError::ParseError { error, file: file_name.to_string() }),
            Err(ZipError::FileNotFound) => Err(SDELoadError::ArchiveFileNotFound(file_name.to_string())),
            Err(err) => Err(SDELoadError::MalformedZip(err))
        }
    }

    #[derive(Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
    #[serde(deny_unknown_fields)]
    pub enum Never {}

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct SDELocalizedString {
        pub en: Option<String>, // Almost always present, maybe replace with a specific default value?
        pub de: Option<String>,
        pub es: Option<String>,
        pub fr: Option<String>,
        pub ja: Option<String>,
        pub ko: Option<String>,
        pub ru: Option<String>,
        pub zh: Option<String>,
        pub it: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]   // TODO: Put these behind a cargo feature for strict-mode
    pub struct InvFlag {
        pub flagID: ids::FlagID,
        pub flagName: String,
        pub flagText: String,
        pub orderID: u32
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct InvItem {
        pub itemID: ids::ItemID,
        pub flagID: ids::FlagID,
        pub locationID: ids::LocationID,
        pub ownerID: u32,
        pub quantity: i32,
        pub typeID: i32
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct InvName {
        pub itemID: ids::ItemID,
        pub itemName: String
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct InvPosition {
        pub itemID: ids::ItemID,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub pitch: Option<f64>,
        pub yaw: Option<f64>,
        pub roll: Option<f64>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct InvUniqueName {
        pub groupID: ids::GroupID,
        pub itemID: ids::ItemID,
        pub itemName: String
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StaStation {
        pub stationID: ids::StationID,
        pub stationName: String,
        pub stationTypeID: ids::TypeID,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub constellationID: ids::ConstellationID,
        pub solarSystemID: ids::SolarSystemID,
        pub corporationID: ids::CorporationID,
        pub regionID: ids::RegionID,
        pub dockingCostPerVolume: f64,
        pub maxShipVolumeDockable: f64,
        pub officeRentalCost: f64,
        pub operationID: ids::StationOperationID,
        pub reprocessingEfficiency: f64,
        pub reprocessingHangarFlag: u32,
        pub reprocessingStationsTake: f64,
        pub security: f64,
    }

    #[derive(Debug)]
    pub struct BSD {
        pub inv_flags: HashMap<ids::ItemID, InvFlag>,
        pub inv_items: HashMap<ids::ItemID, InvItem>,
        pub inv_names: HashMap<ids::ItemID, InvName>,
        pub inv_positions: HashMap<ids::ItemID, InvPosition>,
        pub inv_unique_names: HashMap<ids::ItemID, InvUniqueName>,
        pub sta_stations: HashMap<ids::StationID, StaStation>,
    }

    pub(crate) fn do_load_bsd<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<BSD, SDELoadError> {
        Ok(BSD {
            inv_flags: load_file(
                archive,
                "invFlags.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<InvFlag>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|flag| (flag.flagID, flag))
                            .collect()
                    })
            )?,
            inv_items: load_file(
                archive,
                "invItems.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<InvItem>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|item| (item.itemID, item))
                            .collect()
                    })
            )?,
            inv_names: load_file(
                archive,
                "invNames.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<InvName>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|item| (item.itemID, item))
                            .collect()
                    })
            )?,
            inv_positions: load_file(
                archive,
                "invPositions.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<InvPosition>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|item| (item.itemID, item))
                            .collect()
                    })
            )?,
            inv_unique_names: load_file(
                archive,
                "invUniqueNames.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<InvUniqueName>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|item| (item.itemID, item))
                            .collect()
                    })
            )?,
            sta_stations: load_file(
                archive,
                "staStations.yaml",
                |f| serde_yaml_ng::from_reader::<_, Vec<StaStation>>(f)
                    .map(|vec| {
                        vec.into_iter()
                            .map(|station| (station.stationID, station))
                            .collect()
                    })
            )?,
        })
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Agent {
        pub agentTypeID: ids::TypeID,
        pub corporationID: ids::CorporationID,
        pub divisionID: ids::DivisionID,
        pub isLocator: bool,
        pub level: i32,
        pub locationID: ids::LocationID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AgentInSpace {
        pub dungeonID: ids::DungeonID,
        pub solarSystemID: ids::SolarSystemID,
        pub spawnPointID: ids::SpawnPointID,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Ancestry {
        pub bloodlineID: ids::BloodlineID,
        pub charisma: i32,
        pub intelligence: i32,
        pub memory: i32,
        pub perception: i32,
        pub willpower: i32,
        pub descriptionID: SDELocalizedString,
        pub iconID: Option<ids::IconID>,
        pub nameID: SDELocalizedString,
        pub shortDescription: Option<String>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Bloodline {
        pub corporationID: ids::CorporationID,
        pub descriptionID: SDELocalizedString,
        pub iconID: Option<ids::IconID>,
        pub nameID: SDELocalizedString,
        pub raceID: ids::RaceID,
        pub charisma: i32,
        pub intelligence: i32,
        pub memory: i32,
        pub perception: i32,
        pub willpower: i32,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Blueprint {
        pub blueprintTypeID: ids::TypeID,
        pub maxProductionLimit: i32,
        pub activities: BlueprintActivities
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct BlueprintActivities {
        pub copying: Option<BPActivity>,
        pub manufacturing: Option<BPActivity>,
        pub research_material: Option<BPActivity>,
        pub research_time: Option<BPActivity>,
        pub invention: Option<BPActivity>,
        pub reaction: Option<BPActivity>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct BPActivity {
        #[serde(deserialize_with="deserialize_activity_materials", default)]
        pub materials: HashMap<ids::TypeID, u32>,
        #[serde(deserialize_with="deserialize_activity_products", default)]
        pub products: HashMap<ids::TypeID, (u32, f64)>,
        #[serde(deserialize_with="deserialize_activity_skills", default)]
        pub skills: HashMap<ids::TypeID, numbers::SkillLevel>,
        pub time: u32
    }
    fn deserialize_activity_materials<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HashMap<ids::TypeID, u32>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        #[serde(deny_unknown_fields)]
        pub struct BPMaterial {
            typeID: ids::TypeID,
            quantity: u32
        }

        <Vec<BPMaterial>>::deserialize(deserializer)
            .map(|list| {
                    list.into_iter()
                        .map(|BPMaterial { typeID, quantity }| (typeID, quantity))
                        .collect()
            })
    }
    fn deserialize_activity_products<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HashMap<ids::TypeID, (u32, f64)>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        #[serde(deny_unknown_fields)]
        pub struct BPProduct {
            typeID: ids::TypeID,
            quantity: u32,
            probability: Option<f64>
        }

        <Vec<BPProduct>>::deserialize(deserializer)
            .map(|list| {
                    list.into_iter()
                        .map(|BPProduct { typeID, quantity, probability }| (typeID, (quantity, probability.unwrap_or(1.0))))
                        .collect()
            })
    }
    fn deserialize_activity_skills<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HashMap<ids::TypeID, numbers::SkillLevel>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        #[serde(deny_unknown_fields)]
        pub struct BPSkill {
            typeID: ids::TypeID,
            level: numbers::SkillLevel,
        }

        <Vec<BPSkill>>::deserialize(deserializer).map(|list| list.into_iter().map(|BPSkill { typeID, level: quantity }| (typeID, quantity)).collect())
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Category {
        pub name: SDELocalizedString,
        pub published: bool,
        pub iconID: Option<ids::IconID>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Certificate {
        pub groupID: ids::GroupID,  // TODO: Double-check that this refers to item-groups
        pub name: String,
        pub description: String,
        #[serde(default)]
        pub recommendedFor: Vec<ids::TypeID>,
        pub skillTypes: HashMap<ids::TypeID, CertificateSkillLevels>
    }
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct CertificateSkillLevels {
        pub basic: numbers::SkillLevel,
        pub standard: numbers::SkillLevel,
        pub improved: numbers::SkillLevel,
        pub advanced: numbers::SkillLevel,
        pub elite: numbers::SkillLevel,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CharacterAttribute {
        pub nameID: SDELocalizedString,
        pub description: String,
        pub iconID: ids::IconID,
        pub notes: String,
        pub shortDescription: String
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ContrabandType {
        pub attackMinSec: f64,
        pub confiscateMinSec: f64,
        pub fineByValue: f64,
        pub standingLoss: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ControlTowerResource {
        pub purpose: u8,
        pub quantity: u32,
        pub resourceTypeID: ids::TypeID,
        pub factionID: Option<ids::FactionID>,  // Fuel required if in faction's space
        pub minSecurityLevel: Option<f64>   // Can't use default here as security can be less than zero.
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CorporationActivity {
        pub nameID: SDELocalizedString
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AttributeCategory {
        pub name: String,
        pub description: Option<String>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Attribute {
        pub attributeID: ids::AttributeID,
        pub categoryID: Option<ids::AttributeCategoryID>,
        pub name: String,
        pub description: Option<String>,
        pub displayNameID: Option<SDELocalizedString>,
        pub tooltipDescriptionID: Option<SDELocalizedString>,
        pub tooltipTitleID: Option<SDELocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub dataType: i32,  // TODO: What's this?
        pub defaultValue: f64,
        pub highIsGood: bool,
        pub published: bool,
        pub stackable: bool,
        pub unitID: Option<EVEUnit>,
        pub chargeRechargeTimeID: Option<u32>,    // TODO: Unknown ID
        pub maxAttributeID: Option<ids::AttributeID>,
        pub minAttributeID: Option<ids::AttributeID>,
        pub displayWhenZero: Option<bool>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Effect {
        pub effectID: ids::EffectID,
        pub effectCategory: ids::EffectCategoryID,
        pub effectName: String,
        pub descriptionID: Option<SDELocalizedString>,
        pub displayNameID: Option<SDELocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub guid: Option<String>,
        pub isAssistance: bool,
        pub isOffensive: bool,
        pub isWarpSafe: bool,
        pub propulsionChance: bool,
        pub published: bool,
        pub rangeChance: bool,
        pub electronicChance: bool,
        pub disallowAutoRepeat: bool,
        pub dischargeAttributeID: Option<ids::AttributeID>,
        pub durationAttributeID: Option<ids::AttributeID>,
        pub trackingSpeedAttributeID: Option<ids::AttributeID>,
        pub falloffAttributeID: Option<ids::AttributeID>,
        pub rangeAttributeID: Option<ids::AttributeID>,
        pub npcUsageChanceAttributeID: Option<ids::AttributeID>,
        pub npcActivationChanceAttributeID: Option<ids::AttributeID>,
        pub fittingUsageChanceAttributeID: Option<ids::AttributeID>,
        pub resistanceAttributeID: Option<ids::AttributeID>,
        pub distribution: Option<i32>,  // TODO: Figure out what this is for
        #[serde(default)]
        pub modifierInfo: Vec<ModifierInfo>,
        pub sfxName: Option<String>,    // TODO: Always the string "None" if present?
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ModifierInfo {
        pub domain: String,
        pub func: String,   // TODO: Figure out values
        pub operation: Option<i32>, // TODO: Figure out values
        pub modifiedAttributeID: Option<ids::AttributeID>,
        pub modifyingAttributeID: Option<ids::AttributeID>,
        pub groupID: Option<ids::GroupID>,
        pub effectID: Option<ids::EffectID>,
        pub skillTypeID: Option<ids::TypeID>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Faction {
        pub nameID: SDELocalizedString,
        pub descriptionID: SDELocalizedString,
        pub iconID: ids::IconID,
        pub shortDescriptionID: Option<SDELocalizedString>,
        pub flatLogo: Option<String>,
        pub flatLogoWithName: Option<String>,
        pub corporationID: Option<ids::CorporationID>,
        pub memberRaces: Vec<ids::RaceID>,
        pub militiaCorporationID: Option<ids::CorporationID>,
        pub sizeFactor: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub uniqueName: bool
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Graphic {
        pub description: Option<String>,
        pub graphicFile: Option<String>,
        pub sofFactionName: Option<String>,
        pub sofHullName: Option<String>,
        pub sofRaceName: Option<String>,
        #[serde(default)]
        pub sofLayout: Vec<String>,
        pub iconInfo: Option<IconInfo>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct IconInfo {
        pub folder: String
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Group {
        pub name: SDELocalizedString,
        pub categoryID: ids::CategoryID,
        pub anchorable: bool,
        pub anchored: bool,
        pub fittableNonSingleton: bool,
        pub published: bool,
        pub useBasePrice: bool,
        pub iconID: Option<ids::IconID>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Icon {
        pub description: Option<String>,
        pub iconFile: String,
        pub obsolete: Option<bool>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MarketGroup {
        pub nameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>,
        pub hasTypes: bool,
        pub iconID: Option<ids::IconID>,
        pub parentGroupID: Option<ids::MarketGroupID>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MetaGroup {
        pub color: Option<[f64; 4]>, // TODO: Check order, RGBA?
        pub nameID: SDELocalizedString,
        pub iconID: Option<ids::IconID>,
        pub iconSuffix: Option<String>,
        pub descriptionID: Option<SDELocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCorporationDivision {
        pub internalName: String,
        pub leaderTypeNameID: SDELocalizedString,
        pub description: Option<String>,
        pub nameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCorporation {
        pub nameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub ceoID: Option<ids::CharacterID>,
        pub deleted: bool,
        pub extent: String,
        pub hasPlayerPersonnelManager: bool,
        pub initialPrice: f64,
        pub memberLimit: i32,
        pub minSecurity: f64,
        pub minimumJoinStanding: f64,
        pub publicShares: u64,
        pub sendCharTerminationMessage: bool,
        pub shares: u64,
        pub size: String,
        pub stationID: Option<ids::StationID>,
        pub taxRate: f64,
        pub tickerName: String,
        pub uniqueName: bool,
        pub corporationTrades: Option<HashMap<ids::TypeID, f64>>,
        pub allowedMemberRaces: Option<Vec<ids::RaceID>>,
        pub enemyID: Option<ids::CorporationID>,
        pub factionID: Option<ids::FactionID>,
        pub friendID: Option<ids::CorporationID>,
        pub lpOfferTables: Option<Vec<u32>>,    // TODO: Assign ID type
        pub divisions: Option<HashMap<ids::DivisionID, CorporationDivision>>,
        pub investors: Option<HashMap<ids::CorporationID, i32>>,
        pub mainActivityID: Option<i32>,    // TODO: Assign ID type, probably station activity ID?
        pub secondaryActivityID: Option<i32>,    // TODO: Assign ID type, probably station activity ID?
        pub raceID: Option<ids::RaceID>,
        pub sizeFactor: Option<f64>,
        pub solarSystemID: Option<ids::SolarSystemID>,
        pub exchangeRates: Option<HashMap<ids::CorporationID, f64>>,
        pub url: Option<String> // currently always empty-string
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CorporationDivision {
        pub divisionNumber: i32,
        pub leaderID: ids::CharacterID,
        pub size: i32
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(untagged)]
    #[serde(deny_unknown_fields)]
    pub enum PlanetResource {
        ResourcePlanet { power: i32, workforce: i32 },
        Star { power: i32, },
        ReagentPlanet {
            cycle_minutes: u32,
            harvest_silo_max: u32,
            maturation_cycle_minutes: u32,
            maturation_percent: u32,
            mature_silo_max: f64,
            reagent_harvest_amount: u32,
            reagent_type_id: ids::TypeID
        }
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetSchematic {
        pub cycleTime: u32,
        pub nameID: SDELocalizedString,
        pub pins: Vec<ids::TypeID>,
        pub input: HashMap<ids::TypeID, u32>,
        pub output: HashMap<ids::TypeID, u32>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CharacterRace {
        pub nameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub shipTypeID: Option<ids::TypeID>, // Corvette/"Rookie ship"
        pub skills: Option<HashMap<ids::TypeID, numbers::SkillLevel>>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SkinLicense {
        pub duration: i32,
        pub licenseTypeID: ids::TypeID,
        pub skinID: ids::SkinID,
        pub isSingleUse: Option<bool>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SkinMaterial {
        pub displayNameID: ids::LocalizationStringID,
        pub materialSetID: ids::MaterialSetID,
        pub skinMaterialID: ids::SkinMaterialID,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Skin {
        pub allowCCPDevs: bool,
        pub internalName: String,
        pub skinID: ids::SkinID,
        pub skinMaterialID: ids::SkinMaterialID,
        pub types: Vec<ids::TypeID>,
        pub visibleSerenity: bool,
        pub visibleTranquility: bool,
        pub isStructureSkin: Option<bool>,
        pub skinDescription: Option<String>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SovereigntyUpgrade {
        pub power_allocation: i32,
        pub workforce_allocation: i32,
        pub mutually_exclusive_group: String,
        pub fuel_type_id: Option<ids::TypeID>,
        pub fuel_startup_cost: Option<i32>,
        pub fuel_hourly_upkeep: Option<i32>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StationOperation {
        pub activityID: ids::StationActivityID,
        pub border: f64,
        pub corridor: f64,
        pub fringe: f64,
        pub hub: f64,
        pub operationNameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>,
        pub ratio: f64,
        pub manufacturingFactor: f64,
        pub researchFactor: f64,
        pub services: Vec<ids::StationServiceID>,
        pub stationTypes: Option<HashMap<u32, ids::TypeID>>,    // TODO: Figure out key value
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StationService {
        pub serviceNameID: SDELocalizedString,
        pub descriptionID: Option<SDELocalizedString>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TournamentRuleSet {
        pub banned: TournamentBans,
        pub maximumPilotsMatch: i32,
        pub maximumPointsMatch: i32,
        pub ruleSetID: String,
        pub ruleSetName: String,
        pub points: TournamentPoints
    }
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TournamentBans {
        pub groups: Vec<ids::GroupID>,
        pub types: Vec<ids::TypeID>
    }
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TournamentPoints {
        pub groups: HashMap<ids::GroupID, i32>,
        pub types: HashMap<ids::TypeID, i32>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeDogma {
        pub dogmaAttributes: HashMap<ids::AttributeID, f64>,
        pub dogmaEffects: HashMap<ids::EffectID, bool>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Type {
        pub groupID: ids::GroupID,
        pub name: SDELocalizedString,
        pub published: bool,
        pub description: Option<SDELocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub graphicID: Option<ids::GraphicID>,
        pub mass: Option<f64>,
        pub radius: Option<f64>,
        pub volume: Option<f64>,
        pub soundID: Option<ids::SoundID>,
        pub raceID: Option<ids::RaceID>,
        pub sofFactionName: Option<String>,
        pub sofMaterialSetID: Option<u32>,  // TODO: Figure out id, probably ids::MaterialSetID?
        #[serde(default)]   // Explicit default->None as we use deserialize_with
        #[serde(deserialize_with = "deserialize_id_or_float")] // Sometimes written out as a float, so custom parser
        pub metaGroupID: Option<ids::MetaGroupID>,
        pub marketGroupID: Option<ids::MarketGroupID>,
        pub variationParentTypeID: Option<ids::TypeID>,
        pub factionID: Option<ids::FactionID>,
        pub basePrice: Option<f64>,
        pub capacity: Option<f64>,
        pub masteries: Option<HashMap<u8, Vec<ids::CertificateID>>>,
        pub traits: Option<TypeTraits>,
        pub portionSize: i32,
    }

    fn deserialize_id_or_float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum IDorFloat { ID(u32), FLOAT(f64) }
        <Option<IDorFloat>>::deserialize(deserializer).map(|opt| opt.map(|v| match v { IDorFloat::ID(id) => id, IDorFloat::FLOAT(f) => f as u32 }))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeTraits {   // Kinds of bonuses may be omitted, an empty collection is given for those
        pub iconID: Option<ids::IconID>,
        #[serde(default)]
        pub miscBonuses: Vec<TypeTraitBonus>,
        #[serde(default)]
        pub roleBonuses: Vec<TypeTraitBonus>,
        #[serde(default)]
        #[serde(rename = "types")]
        pub skillBonuses: HashMap<ids::TypeID, Vec<TypeTraitBonus>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeTraitBonus {
        pub bonusText: SDELocalizedString,
        pub importance: i32,
        pub bonus: Option<f64>,
        pub unitID: Option<EVEUnit>,
        pub isPositive: Option<bool>
    }

    #[derive(Debug)]
    pub struct FSD {
        pub agents: HashMap<ids::AgentID, Agent>,
        pub agents_in_space: HashMap<ids::AgentID, AgentInSpace>,
        pub ancestries: HashMap<ids::AncestryID, Ancestry>,
        pub bloodlines: HashMap<ids::BloodlineID, Bloodline>,
        pub blueprints: HashMap<ids::TypeID, Blueprint>,
        pub categories: HashMap<ids::CategoryID, Category>,
        pub certificates: HashMap<ids::CertificateID, Certificate>,
        pub character_attributes: HashMap<ids::CharacterAttributeID, CharacterAttribute>,
        pub contraband_types: HashMap<ids::TypeID, HashMap<ids::FactionID, ContrabandType>>,
        pub control_tower_resources: HashMap<ids::TypeID, Vec<ControlTowerResource>>,
        pub corporation_activities: HashMap<ids::CorporationActivityID, CorporationActivity>,
        pub dogma_attribute_categories: HashMap<ids::AttributeCategoryID, AttributeCategory>,
        pub dogma_attributes: HashMap<ids::AttributeID, Attribute>,
        pub dogma_effects: HashMap<ids::EffectID, Effect>,
        pub factions: HashMap<ids::FactionID, Faction>,
        pub graphics: HashMap<ids::GraphicID, Graphic>,
        pub groups: HashMap<ids::GroupID, Group>,
        pub icons: HashMap<ids::IconID, Icon>,
        pub market_groups: HashMap<ids::MarketGroupID, MarketGroup>,
        pub meta_groups: HashMap<ids::MetaGroupID, MetaGroup>,
        pub npc_corporation_divisions: HashMap<ids::DivisionID, NpcCorporationDivision>,
        pub npc_corporations: HashMap<ids::CorporationID, NpcCorporation>,
        pub planet_resources: HashMap<ids::ItemID, PlanetResource>,
        pub planet_schematics: HashMap<ids::PlanetSchematicID, PlanetSchematic>,
        pub character_races: HashMap<ids::RaceID, CharacterRace>,
        pub research_agents: HashMap<ids::AgentID, Vec<ids::TypeID>>,
        pub skin_licenses: HashMap<ids::TypeID, SkinLicense>,
        pub skin_materials: HashMap<ids::SkinMaterialID, SkinMaterial>,
        pub skins: HashMap<ids::SkinID, Skin>,
        pub sovereignty_upgrades: HashMap<ids::TypeID, SovereigntyUpgrade>,
        pub station_operations: HashMap<ids::StationOperationID, StationOperation>,
        pub station_services: HashMap<ids::StationServiceID, StationService>,
        pub tournament_rule_sets: HashMap<String, TournamentRuleSet>,
        pub translation_languages: HashMap<String, String>,
        pub type_dogma: HashMap<ids::TypeID, TypeDogma>,
        pub type_materials: HashMap<ids::TypeID, HashMap<ids::TypeID, u32>>,
        pub types: HashMap<ids::TypeID, Type>
    }

    pub(crate) fn do_load_fsd<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<FSD, SDELoadError> {
        Ok(FSD {
            agents: load_file(archive, "agents.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            agents_in_space: load_file(archive, "agentsInSpace.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            ancestries: load_file(archive, "ancestries.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            bloodlines: load_file(archive, "bloodlines.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            blueprints: load_file(archive, "blueprints.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            categories: load_file(archive, "categories.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            certificates: load_file(archive, "certificates.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            character_attributes: load_file(archive, "characterAttributes.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            contraband_types: load_file(archive, "contrabandTypes.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[serde(deny_unknown_fields)]
                pub struct ContrabandFaction { factions: HashMap<ids::FactionID, ContrabandType> }
                serde_yaml_ng::from_reader::<_, HashMap<ids::TypeID, ContrabandFaction>>(f)
                    .map(|m| m.into_iter().map(|(k, v)| (k, v.factions)).collect()) // Unwrap ContrabandFaction, this isn't efficient but writing a dedicated Deserializer is :effort:
            })?,
            control_tower_resources: load_file(archive, "controlTowerResources.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[serde(deny_unknown_fields)]
                pub struct ControlTower { resources: Vec<ControlTowerResource> }
                serde_yaml_ng::from_reader::<_, HashMap<ids::TypeID, ControlTower>>(f)
                    .map(|m| m.into_iter().map(|(k, v)| (k, v.resources)).collect()) // Unwrap ControlTower, this isn't efficient but writing a dedicated Deserializer is :effort:
            })?,
            corporation_activities: load_file(archive, "corporationActivities.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            dogma_attribute_categories: load_file(archive, "dogmaAttributeCategories.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            dogma_attributes: load_file(archive, "dogmaAttributes.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            dogma_effects: load_file(archive, "dogmaEffects.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            factions: load_file(archive, "factions.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            graphics: load_file(archive, "graphicIDs.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            groups: load_file(archive, "groups.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            icons: load_file(archive, "iconIDs.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            market_groups: load_file(archive, "marketGroups.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            meta_groups: load_file(archive, "metaGroups.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            npc_corporation_divisions: load_file(archive, "npcCorporationDivisions.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            npc_corporations: load_file(archive, "npcCorporations.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            planet_resources: load_file(archive, "planetResources.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            planet_schematics: load_file(archive, "planetSchematics.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct PISchematicType {
                    isInput: bool,
                    quantity: u32
                }

                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct PlanetSchematicYaml {
                    cycleTime: u32,
                    nameID: SDELocalizedString,
                    pins: Vec<ids::TypeID>,
                    types: HashMap<ids::TypeID, PISchematicType>
                }
                serde_yaml_ng::from_reader::<_, HashMap<ids::PlanetSchematicID, PlanetSchematicYaml>>(f)
                    .map(|m| {
                        // Replace PlanetSchematicYaml with the more convenient PlanetSchematic
                        m.into_iter().map(|(k, v)| {
                            (k, PlanetSchematic {
                                cycleTime: v.cycleTime,
                                nameID: v.nameID,
                                pins: v.pins,
                                input: v.types.iter().filter_map(|(type_id, t)| if t.isInput { Some((*type_id, t.quantity)) } else { None }).collect(),
                                output: v.types.iter().filter_map(|(type_id, t)| if !t.isInput { Some((*type_id, t.quantity)) } else { None }).collect(),
                            })
                        }).collect()
                    })
            })?,
            character_races: load_file(archive, "races.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            research_agents: load_file(archive, "researchAgents.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[serde(deny_unknown_fields)]
                pub struct ResearchAgent { skills: Vec<ResearchType> }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct ResearchType { typeID: ids::TypeID }
                serde_yaml_ng::from_reader::<_, HashMap<ids::TypeID, ResearchAgent>>(f)
                    .map(|m| m.into_iter().map(|(k, v)| (k, v.skills.into_iter().map(|t| t.typeID).collect())).collect()) // Unwrap ResearchAgent
            })?,
            skin_licenses: load_file(archive, "skinLicenses.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            skin_materials: load_file(archive, "skinMaterials.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            skins: load_file(archive, "skins.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            sovereignty_upgrades: load_file(archive, "sovereigntyUpgrades.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            station_operations: load_file(archive, "stationOperations.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            station_services: load_file(archive, "stationServices.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            tournament_rule_sets: load_file(archive, "tournamentRuleSets.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct TournamentRuleSetYaml {
                    pub banned: TournamentBans,
                    pub maximumPilotsMatch: i32,
                    pub maximumPointsMatch: i32,
                    pub ruleSetID: String,
                    pub ruleSetName: String,
                    pub points: TournamentPointsYaml
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct TournamentPointsYaml {
                    pub groups: Vec<PointsGroup>,
                    pub types: Vec<PointsType>
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct PointsGroup {
                    pub points: i32,
                    pub groupID: ids::GroupID
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct PointsType {
                    pub points: i32,
                    pub typeID: ids::TypeID
                }

                serde_yaml_ng::from_reader::<_, Vec<TournamentRuleSetYaml>>(f)
                    .map(|list| {
                        list.into_iter().map(|rs| {
                            (rs.ruleSetID.clone(), TournamentRuleSet {
                                banned: rs.banned,
                                maximumPilotsMatch: rs.maximumPilotsMatch,
                                maximumPointsMatch: rs.maximumPointsMatch,
                                ruleSetID: rs.ruleSetID,
                                ruleSetName: rs.ruleSetName,
                                points: TournamentPoints {
                                    groups: rs.points.groups.into_iter().map(|p| (p.groupID, p.points)).collect(),
                                    types: rs.points.types.into_iter().map(|p| (p.typeID, p.points)).collect()
                                },
                            })
                        }).collect()
                    })
            })?,
            translation_languages: load_file(archive, "translationLanguages.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
            type_dogma: load_file(archive, "typeDogma.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct TypeDogmaYaml {
                    dogmaAttributes: Vec<DogmaAttributeYaml>,
                    dogmaEffects: Vec<DogmaEffectYaml>,
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct DogmaAttributeYaml {
                    attributeID: ids::AttributeID,
                    value: f64,
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct DogmaEffectYaml {
                    effectID: ids::EffectID,
                    isDefault: bool
                }

                serde_yaml_ng::from_reader::<_, HashMap<ids::TypeID, TypeDogmaYaml>>(f)
                    .map(|map| {
                        map.into_iter().map(|(type_id, dogma)| {
                            (
                                type_id,
                                TypeDogma {
                                    dogmaAttributes: dogma.dogmaAttributes.into_iter().map(|a| (a.attributeID, a.value)).collect(),
                                    dogmaEffects: dogma.dogmaEffects.into_iter().map(|e| (e.effectID, e.isDefault)).collect(),
                                }
                            )
                        }).collect()
                    })
            })?,
            type_materials: load_file(archive, "typeMaterials.yaml", |f| {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct TypeMaterialsYaml {
                    materials: Vec<TypeMaterial>,
                }
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                pub struct TypeMaterial {
                    materialTypeID: ids::AttributeID,
                    quantity: u32,
                }

                serde_yaml_ng::from_reader::<_, HashMap<ids::TypeID, TypeMaterialsYaml>>(f)
                    .map(|map| {
                        map.into_iter().map(|(type_id, materials)| {
                            (type_id, materials.materials.into_iter().map(|m| (m.materialTypeID, m.quantity)).collect())
                        }).collect()
                    })
            })?,
            types: load_file(archive, "types.yaml", |f| serde_yaml_ng::from_reader::<_, _>(f))?,
        })
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SolarSystem {
        #[serde(default)]   // Not contained in the YAML, we backfill this value later
        pub constellationID: ids::ConstellationID,
        pub luminosity: f64,
        pub center: [f64; 3],  // TODO: Document the axes on these
        pub max: [f64; 3],
        pub min: [f64; 3],
        pub radius: f64,
        pub security: f64,
        pub securityClass: Option<String>,
        pub solarSystemID: ids::SolarSystemID,
        pub solarSystemNameID: ids::LocalizationStringID,
        pub descriptionID: Option<ids::LocalizationStringID>,
        pub sunTypeID: Option<ids::TypeID>,
        pub wormholeClassID: Option<ids::WormholeClassID>,
        pub factionID: Option<ids::FactionID>,
        pub star: Option<Star>,
        #[serde(default)]
        pub planets: HashMap<ids::ItemID, Planet>,
        #[serde(default)]
        pub stargates: HashMap<ids::ItemID, Stargate>,
        pub disallowedAnchorCategories: Option<Vec<ids::CategoryID>>,
        pub disallowedAnchorGroups: Option<Vec<ids::GroupID>>,
        pub visualEffect: Option<String>,
        pub secondarySun: Option<SecondarySun>,
        pub border: bool,
        pub corridor: bool,
        pub fringe: bool,
        pub hub: bool,
        pub regional: bool,
        pub international: bool,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Star {
        pub id: ids::ItemID,
        pub radius: f64,
        pub statistics: StarStatistics,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SecondarySun {
        pub typeID: ids::TypeID,
        pub itemID: ids::ItemID,
        pub effectBeaconTypeID: ids::TypeID,
        pub position: [f64; 3],  // TODO: Document the axes on these
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StarStatistics {
        pub age: f64,
        pub life: f64,
        pub locked: bool,
        pub luminosity: f64,
        pub radius: f64,
        pub spectralClass: String,
        pub temperature: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Planet {
        pub position: [f64; 3],  // TODO: Document the axes on these
        pub radius: f64,
        pub typeID: ids::TypeID,
        pub planetNameID: Option<ids::LocalizationStringID>,
        pub celestialIndex: i32,
        pub planetAttributes: PlanetAttributes,
        pub statistics: CelestialStatistics,
        #[serde(default)]
        pub moons: HashMap<ids::ItemID, Moon>,
        #[serde(default)]
        pub asteroidBelts: HashMap<ids::ItemID, AsteroidBelt>,
        #[serde(default)]
        pub npcStations: HashMap<ids::StationID, NpcStation>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetAttributes {    // TODO: ID types
        pub heightMap1: u32,
        pub heightMap2: u32,
        pub population: bool,
        pub shaderPreset: u32
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Moon {
        pub position: [f64; 3],  // TODO: Document the axes on these
        pub radius: f64,
        pub typeID: ids::TypeID,
        pub moonNameID: Option<ids::LocalizationStringID>,
        pub planetAttributes: PlanetAttributes,
        pub statistics: Option<CelestialStatistics>,
        #[serde(default)]
        pub npcStations: HashMap<ids::StationID, NpcStation>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcStation {
        pub graphicID: ids::GraphicID,
        pub typeID: ids::TypeID,
        pub isConquerable: bool,
        pub operationID: ids::StationOperationID,
        pub ownerID: ids::CorporationID,
        pub position: [f64; 3],  // TODO: Document the axes on these
        pub reprocessingEfficiency: f64,
        pub reprocessingHangarFlag: i32,
        pub reprocessingStationsTake: f64,
        pub useOperationName: bool
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AsteroidBelt {
        pub position: [f64; 3],  // TODO: Document the axes on these
        pub asteroidBeltNameID: Option<ids::LocalizationStringID>,
        pub statistics: Option<CelestialStatistics>,
        pub typeID: ids::TypeID
    }


    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CelestialStatistics {
        pub density: f64,
        pub eccentricity: f64,
        pub escapeVelocity: f64,
        pub fragmented: bool,
        pub life: f64,
        pub locked: bool,
        pub massDust: f64,
        pub massGas: f64,
        pub orbitPeriod: f64,
        pub orbitRadius: f64,
        pub pressure: f64,
        pub radius: f64,
        pub rotationRate: f64,
        pub spectralClass: String,
        pub surfaceGravity: f64,
        pub temperature: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Stargate {
        pub destination: ids::ItemID,
        pub position: [f64; 3],  // TODO: Document the axes on these
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Constellation {
        pub constellationID: ids::ConstellationID,
        #[serde(default)]
        pub regionID: ids::RegionID,
        pub factionID: Option<ids::FactionID>,
        pub center: [f64; 3],  // TODO: Document the axes on these
        pub max: [f64; 3],
        pub min: [f64; 3],
        pub nameID: ids::LocalizationStringID,
        pub radius: f64,
        pub wormholeClassID: Option<ids::WormholeClassID>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Region {
        pub regionID: ids::RegionID,
        pub descriptionID: Option<ids::LocalizationStringID>,
        pub factionID: Option<ids::FactionID>,
        pub center: [f64; 3],  // TODO: Document the axes on these
        pub max: [f64; 3],
        pub min: [f64; 3],
        pub nameID: ids::LocalizationStringID,
        pub nebula: u32,    // TODO: Assign ID type
        pub wormholeClassID: Option<ids::WormholeClassID>
    }
    
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Landmark {
        pub landmarkNameID: ids::LocalizationStringID,
        pub descriptionID: ids::LocalizationStringID,
        pub iconID: Option<ids::IconID>,
        pub locationID: Option<ids::LocationID>,
        pub position: [f64; 3],  // TODO: Document the axes on these
    }

    #[derive(Debug)]
    pub struct Universe {
        pub regions: HashMap<ids::RegionID, Region>,
        pub constellations: HashMap<ids::ConstellationID, Constellation>,
        pub solarsystems: HashMap<ids::SolarSystemID, SolarSystem>,
        pub landmarks: HashMap<ids::LandmarkID, Landmark>,
    }

    pub(crate) fn do_load_universe<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Universe, SDELoadError> {
        let mut system_map = HashMap::<String, Vec<SolarSystem>>::new();
        let mut constellation_map = HashMap::<String, Vec<(Constellation, String)>>::new();
        let mut region_map = HashMap::<String, Vec<(Region, String)>>::new();
        let mut landmarks = HashMap::<ids::LandmarkID, Landmark>::new();

        for idx in 0..archive.len() {
            let filename = archive.name_for_index(idx).unwrap().to_string();
            if let Some(path) = filename.strip_suffix("/solarsystem.yaml") {
                let [_system_name, constellation_name] = path.rsplit('/').array_chunks().next().ok_or(SDELoadError::MalformedSDE)?;

                let system: SolarSystem = match archive.by_index(idx) {
                    Ok(file) => serde_yaml_ng::from_reader::<_, _>(file).map_err(|error| SDELoadError::ParseError { error, file: filename.clone() }),
                    Err(ZipError::FileNotFound) => Err(SDELoadError::ArchiveFileNotFound(filename.clone())),
                    Err(err) => Err(SDELoadError::MalformedZip(err))
                }?;

                system_map.entry(constellation_name.to_string()).or_default().push(system)
            } else if let Some(path) = filename.strip_suffix("/constellation.yaml") {
                let [constellation_name, region_name] = path.rsplit('/').array_chunks().next().ok_or(SDELoadError::MalformedSDE)?;

                let constellation: Constellation = match archive.by_index(idx) {
                    Ok(file) => serde_yaml_ng::from_reader::<_, _>(file).map_err(|error| SDELoadError::ParseError { error, file: filename.clone() }),
                    Err(ZipError::FileNotFound) => Err(SDELoadError::ArchiveFileNotFound(filename.clone())),
                    Err(err) => Err(SDELoadError::MalformedZip(err))
                }?;

                constellation_map.entry(region_name.to_string()).or_default()
                    .push((constellation, constellation_name.to_string()))
            } else if let Some(path) = filename.strip_suffix("/region.yaml") {
                let [region_name, cluster_name] = path.rsplit('/').array_chunks().next().ok_or(SDELoadError::MalformedSDE)?;

                let region: Region = match archive.by_index(idx) {
                    Ok(file) => serde_yaml_ng::from_reader::<_, _>(file).map_err(|error| SDELoadError::ParseError { error, file: filename.clone() }),
                    Err(ZipError::FileNotFound) => Err(SDELoadError::ArchiveFileNotFound(filename.clone())),
                    Err(err) => Err(SDELoadError::MalformedZip(err))
                }?;

                region_map.entry(cluster_name.to_string()).or_default()
                    .push((region, region_name.to_string()))
            } else if filename.ends_with("/landmarks.yaml") {
                landmarks = match archive.by_index(idx) {
                    Ok(file) => serde_yaml_ng::from_reader::<_, _>(file).map_err(|error| SDELoadError::ParseError { error, file: filename.clone() }),
                    Err(ZipError::FileNotFound) => Err(SDELoadError::ArchiveFileNotFound(filename.clone())),
                    Err(err) => Err(SDELoadError::MalformedZip(err))
                }?;
            }
        }

        let mut universe = Universe {
            regions: HashMap::new(),
            constellations: HashMap::new(),
            solarsystems: HashMap::new(),
            landmarks,
        };

        let mut region_ids = HashMap::<String, ids::RegionID>::new();
        let mut constellation_ids = HashMap::<String, ids::RegionID>::new();

        for (_cluster_name, regions) in region_map { // TODO: Use cluster names
            for (region, name) in regions {
                region_ids.insert(name, region.regionID);
                universe.regions.insert(region.regionID, region);
            }
        }

        for (region_name, constellations) in constellation_map {
            let region_id = *region_ids.get(&region_name).ok_or(SDELoadError::MalformedSDE)?;
            for (mut constellation, name) in constellations {
                constellation.regionID = region_id;
                constellation_ids.insert(name, constellation.constellationID);
                universe.constellations.insert(constellation.constellationID, constellation);
            }
        }

        for (constellation_name, systems) in system_map {
            let constellation_id = *constellation_ids.get(&constellation_name).ok_or(SDELoadError::MalformedSDE)?;
            for mut system in systems {
                system.constellationID = constellation_id;
                universe.solarsystems.insert(system.solarSystemID, system);
            }
        }

        Ok(universe)
    }

    #[derive(Debug)]
    pub struct SDE {
        pub bsd: BSD,
        pub fsd: FSD,
        pub universe: Universe
    }

    pub fn load_all<R: Read + Seek>(input: R) -> Result<SDE, SDELoadError> {
        let mut archive = ZipArchive::new(input)?;

        Ok(SDE {
            bsd: do_load_bsd(&mut archive)?,
            fsd: do_load_fsd(&mut archive)?,
            universe: do_load_universe(&mut archive)?,
        })
    }
}

#[cfg(feature="update")]
pub mod update {
    use std::fmt::{Debug, Formatter};
    use std::{fs, io};
    use std::fs::File;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};

    pub const CHECKSUM_URL: &'static str = "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/checksum";

    #[derive(Copy, Clone, Default, Eq, PartialEq)]
    pub struct SDEChecksums {
        // Fields are stack allocated strings and must be utf-8. Should only be set by copying bytes out of a `str`
        // Derive-default is safe here, as rust strings may contain NUL
        // Strings of all-NUL indicate there is no known checksum
        sde: [u8; 32],
        fsd: [u8; 32],
        bsd: [u8; 32],
        universe: [u8; 32]
    }

    impl Debug for SDEChecksums {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SDEChecksums")
                .field("sde", &std::str::from_utf8(&self.sde).unwrap())  // Could be replaced with unchecked conversion as utf-8 validity is an invariant, but it's only Debug so :meh:
                .field("fsd", &std::str::from_utf8(&self.fsd).unwrap())
                .field("bsd", &std::str::from_utf8(&self.bsd).unwrap())
                .field("universe", &std::str::from_utf8(&self.universe).unwrap())
                .finish()
        }
    }

    impl SDEChecksums {
        pub fn sde(&self) -> &str {
            std::str::from_utf8(&self.sde).unwrap()
        }

        pub fn fsd(&self) -> &str {
            std::str::from_utf8(&self.fsd).unwrap()
        }

        pub fn bsd(&self) -> &str {
            std::str::from_utf8(&self.bsd).unwrap()
        }

        pub fn universe(&self) -> &str {
            std::str::from_utf8(&self.universe).unwrap()
        }

        pub fn get(&self, kind: SDEKind) -> &str {
            match kind {
                SDEKind::FULL => self.sde(),
                SDEKind::FSD => self.fsd(),
                SDEKind::BSD => self.bsd(),
                SDEKind::UNIVERSE => self.universe()
            }
        }

        pub fn download() -> Result<SDEChecksums, io::Error> {
            let mut checksums = SDEChecksums::default();
            let checksum_text = reqwest::blocking::get(CHECKSUM_URL).map_err(io::Error::other)?.text().map_err(io::Error::other)?;

            for line in checksum_text.lines() {
                let (hex, file) = line.split_once("  ").ok_or_else(|| io::Error::other("malformed checksum file"))?;
                if hex.len() != 32 { Err(io::Error::other("malformed checksum file"))? }

                match file {
                    "sde.zip" => checksums.sde = hex.as_bytes().try_into().unwrap(),  // Unwrap, as we checked the size of 'hex' already
                    "fsd.zip" => checksums.fsd = hex.as_bytes().try_into().unwrap(),  // Unwrap, as we checked the size of 'hex' already
                    "bsd.zip" => checksums.bsd = hex.as_bytes().try_into().unwrap(),  // Unwrap, as we checked the size of 'hex' already
                    "universe.zip" => checksums.universe = hex.as_bytes().try_into().unwrap(),  // Unwrap, as we checked the size of 'hex' already
                    _ => continue
                }
            }

            if
            checksums.sde == [0; 32]
                || checksums.fsd == [0; 32]
                || checksums.bsd == [0; 32]
                || checksums.universe == [0; 32]
            {
                Err(io::Error::other("malformed checksum file"))
            } else {
                Ok(checksums)
            }
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum SDEKind {
        FULL,
        FSD,
        BSD,
        UNIVERSE
    }

    impl SDEKind {
        /// URL for the kind of SDE referred to.
        pub const fn url(self) -> &'static str {
            match self {
                SDEKind::FULL => "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/sde.zip",
                SDEKind::FSD => "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/fsd.zip",
                SDEKind::BSD => "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/bsd.zip",
                SDEKind::UNIVERSE => "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/universe.zip"
            }
        }

        /// Download a new copy of this kind of SDE
        pub fn download<W: io::Write + ?Sized>(&self, destination: &mut W) -> Result<(), io::Error> {
            reqwest::blocking::get(self.url())
                .map_err(io::Error::other)?
                .copy_to(destination)
                .map(|_bytes| ())
                .map_err(io::Error::other)
        }

        /// Default filename for this kind of SDE
        pub const fn filename(self) -> &'static str {
            match self {
                SDEKind::FULL => "sde.zip",
                SDEKind::FSD => "fsd.zip",
                SDEKind::BSD => "bsd.zip",
                SDEKind::UNIVERSE => "universe.zip"
            }
        }

        /// Updates a local copy of the SDE if outdated
        ///
        /// # Arguments
        ///
        /// * `folder_path`: Folder within which files are written
        ///
        /// returns: OK((file_path, true)) if the SDE was updated, Ok((file_path, false)) if it was already up-to-date. Err(io:Error) if an IO error occurred
        pub fn update<P: AsRef<Path>>(&self, folder_path: P) -> Result<(PathBuf, bool), io::Error> {
            let path = folder_path.as_ref();
            if !path.is_dir() { return Err(io::Error::new(ErrorKind::NotADirectory, "SDE update path must be a directory within which the file is written, not a file")); }
            let sde_file = path.join(self.filename());
            let checksum_file = sde_file.with_extension("checksum");

            let checksums = SDEChecksums::download()?;

            let is_fresh =  fs::read_to_string(checksum_file.as_path()).is_ok_and(|s| s == checksums.get(*self)) && sde_file.is_file(); // is_file performs an 'exists' check
            if is_fresh {
                Ok((sde_file, false))
            } else {
                let mut file = File::create(sde_file.as_path())?;
                self.download(&mut file)?;
                fs::write(checksum_file, checksums.get(*self))?;
                Ok((sde_file, true))
            }
        }
    }
}
