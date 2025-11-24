#[cfg(feature = "load")]
pub mod load {
    use std::error::Error;
    use crate::types::{ids, numbers};
    use serde::{Deserialize, Deserializer};
    use std::fmt::{Display, Formatter};
    use std::hash::Hash;
    use std::io;
    use std::io::{BufRead, BufReader, Read, Seek};
    use std::marker::PhantomData;
    use indexmap::IndexMap;
    use serde::de::{DeserializeOwned, SeqAccess, Unexpected, Visitor};
    use zip::result::ZipError;
    use zip::ZipArchive;

    /// Error indicating failure to load SDE
    #[derive(Debug)]
    pub enum SDELoadError {
        /// IO Error while reading from SDE
        IO(io::Error),
        /// An error occurred parsing the .zip file; Archive corrupt?
        Zip(ZipError),
        /// SDE zip file did not contain expected file, did the SDE format change?
        ArchiveFileNotFound(String),
        /// Parsing the JSON content failed, did the SDE schema change?
        ParseError { file: String, entry: usize, error: serde_json::Error}
    }

    impl Display for SDELoadError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                SDELoadError::IO(err) => write!(f, "IO error: {}", err),
                SDELoadError::Zip(err) => write!(f, "Zip error: {}", err),
                SDELoadError::ArchiveFileNotFound(filename) => write!(f, "SDE did not contain expected file: `{}`", filename),
                SDELoadError::ParseError { file, entry, error } => write!(f, "Parse error in `{}` entry {}: {}", file, entry, error),
            }
        }
    }

    impl Error for SDELoadError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                SDELoadError::IO(err) => Some(err),
                SDELoadError::Zip(err) => Some(err),
                SDELoadError::ArchiveFileNotFound(_) => None,
                SDELoadError::ParseError { error, .. } => Some(error)
            }
        }
    }

    impl From<io::Error> for SDELoadError {
        fn from(value: io::Error) -> Self {
            SDELoadError::IO(value)
        }
    }

    /// Load a single file from the zip archive, and parse it to a datatype
    ///
    /// Returns an iterator over each entry
    fn load_file<'a, T: DeserializeOwned, R: Read + Seek>(archive: &'a mut ZipArchive<R>, file_name: &'a str) -> Result<impl Iterator<Item=Result<T, SDELoadError>> + use<'a, T, R>, SDELoadError> {
        let mut str_buf = String::new();
        let mut reader = BufReader::new(
            archive
                .by_name(file_name)
                .map_err(|err| {
                    if let ZipError::FileNotFound = err {
                        SDELoadError::ArchiveFileNotFound(file_name.to_owned())
                    } else {
                        SDELoadError::Zip(err)
                    }
                })?
        );

        let mut entry = 0;
        Ok(std::iter::from_fn(move || { // TODO: Replace with a proper custom iterator implementation to provide better support for skip/nth
            match reader.read_line(&mut str_buf) {
                Ok(0) => None,
                Ok(_) => {
                    entry += 1;
                    let res = serde_json::from_str::<T>(&str_buf).map_err(|error| SDELoadError::ParseError { file: file_name.to_owned(), entry, error });
                    str_buf.clear();
                    Some(res)
                }
                Err(err) => Some(Err(SDELoadError::IO(err))),
            }
        }))
    }

    /// Helper trait for `deserialize_inline_entry_map`
    trait InlineEntry<K> {
        fn key(&self) -> K;
    }

    /// Deserialize a json-array of [`InlineEntry`]-trait values into an IndexMap
    fn deserialize_inline_entry_map<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de> + InlineEntry<K>, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<K, V>, D::Error> {
        struct EntryVisitor<K, V>(PhantomData<K>, PhantomData<V>);
        impl<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de> + InlineEntry<K>> Visitor<'de> for EntryVisitor<K, V> {
            type Value = IndexMap<K, V>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a map encoded as array of flattened entries")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<V>()? {
                    map.insert(InlineEntry::key(&value), value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(EntryVisitor::<K, V>(PhantomData::default(), PhantomData::default()))
    }

    /// Deserialize a json-array of [`ExplicitMapEntry`] values into an IndexMap
    fn deserialize_explicit_entry_map<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de>, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<K, V>, D::Error> {
        struct EntryVisitor<K, V>(PhantomData<K>, PhantomData<V>);
        impl<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de>> Visitor<'de> for EntryVisitor<K, V> {
            type Value = IndexMap<K, V>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a map encoded as array of flattened entries")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<ExplicitMapEntry<K, V>>()? {
                    map.insert(value._key, value._value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(EntryVisitor::<K, V>(PhantomData::default(), PhantomData::default()))
    }

    // Generic types
    /// Helper type for JSON maps that are encoded as arrays of object entries
    #[derive(Deserialize)]
    struct ExplicitMapEntry<K, V> {
        _key: K,
        _value: V
    }

    /// Position of an object, units in metres.
    ///
    /// Up/down, Left/right, Forwards/backwards directions depend on context, see <https://developers.eveonline.com/docs/guides/map-data/> for detailed explanation
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Position {
        pub x: f64,
        pub y: f64,
        pub z: f64
    }

    /// 2D-map position of an object, units in metres.
    ///
    /// Up/down, Left/right directions depend on context, see <https://developers.eveonline.com/docs/guides/map-data/> for detailed explanation
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Position2D {
        pub x: f64,
        pub y: f64
    }

    /// String with multiple language variants
    ///
    /// English is always available. Usually, all other languages are also available
    ///
    /// [`try_*`] methods will return the specified-language version if present, or fall back to the english string.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct LocalizedString {
        /// English
        pub en: String,
        /// German
        pub de: Option<String>,
        /// Spanish
        pub es: Option<String>,
        /// French
        pub fr: Option<String>,
        /// Japanese
        pub ja: Option<String>,
        /// Korean
        pub ko: Option<String>,
        /// Russian
        pub ru: Option<String>,
        /// Chinese
        pub zh: Option<String>
    }

    impl LocalizedString {
        /// German string if available, else English string
        pub fn try_de(&self) -> &str {
            self.de.as_ref().unwrap_or(&self.en)
        }

        /// Spanish string if available, else English string
        pub fn try_es(&self) -> &str {
            self.es.as_ref().unwrap_or(&self.en)
        }

        /// French string if available, else English string
        pub fn try_fr(&self) -> &str {
            self.fr.as_ref().unwrap_or(&self.en)
        }

        /// Japanese string if available, else English string
        pub fn try_ja(&self) -> &str {
            self.ja.as_ref().unwrap_or(&self.en)
        }

        /// Korean string if available, else English string
        pub fn try_ko(&self) -> &str {
            self.ko.as_ref().unwrap_or(&self.en)
        }

        /// Russian string if available, else English string
        pub fn try_ru(&self) -> &str {
            self.ru.as_ref().unwrap_or(&self.en)
        }

        /// Chinese string if available, else English string
        pub fn try_zh(&self) -> &str {
            self.zh.as_ref().unwrap_or(&self.en)
        }
    }

    // SDE Entry types

    /// Agent (Mission NPC) that is located in space, rather than docked in a station
    ///
    /// Additional Agent information is contained in [`NpcCharacter`] data
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AgentInSpace {
        /// CharacterID for this agent
        #[serde(rename="_key")]
        pub agentID: ids::CharacterID,
        /// 'Dungeon' within which the agent is located
        ///
        /// Data about dungeons is not available for EVE 3rd party developers
        pub dungeonID: ids::DungeonID,
        /// SolarSystem in which the Agent is located
        pub solarSystemID: ids::SolarSystemID,
        /// Spawnpoint for agent, no data available for EVE 3rd party developers
        pub spawnPointID: ids::SpawnPointID,
        /// TypeID of the agent's ship (Note: Agent Ships are not the same as the player-flyable ship, and have different TypeIDs)
        pub typeID: ids::TypeID
    }

    pub fn load_agents_in_space<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterID, AgentInSpace), SDELoadError>>, SDELoadError> {
        load_file::<AgentInSpace, R>(archive, "agentsInSpace.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.agentID, entry))))
    }

    /// The different kinds of agent
    ///
    /// See <https://wiki.eveuniversity.org/Agent#Category> for information about the various kinds of Agent
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub enum AgentType {
        NonAgent,
        BasicAgent,
        TutorialAgent,
        ResearchAgent,
        CONCORDAgent,
        GenericStorylineMissionAgent,
        StorylineMissionAgent,
        EventMissionAgent,
        FactionalWarfareAgent,
        EpicArcAgent,
        AuraAgent,
        CareerAgent,
        HeraldryAgent
    }

    /// Helper type for deserializing
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    struct AgentTypeEntry {
        #[serde(rename="_key")]
        agentTypeID: ids::AgentTypeID,
        name: AgentType
    }

    pub fn load_agent_types<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AgentTypeID, AgentType), SDELoadError>>, SDELoadError> {
        load_file::<AgentTypeEntry, R>(archive, "agentTypes.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.agentTypeID, entry.name))))
    }

    /// Character Ancestry; Now-unused character creation element (Removed from player character creation 2021-03-02)
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Ancestry {
        /// Identifier for this Ancestry, see [`ids::AncestryID`]
        #[serde(rename="_key")]
        pub ancestryID: ids::AncestryID,
        /// Bloodline this ancestry is a part of
        pub bloodlineID: ids::BloodlineID,
        /// Skill training attribute modifier
        pub charisma: i32,
        /// Skill training attribute modifier
        pub intelligence: i32,
        /// Skill training attribute modifier
        pub memory: i32,
        /// Skill training attribute modifier
        pub perception: i32,
        /// Skill training attribute modifier
        pub willpower: i32,
        /// Ancestry description as (previously) displayed in game client
        pub description: LocalizedString,
        /// Icon, if specified
        pub iconID: Option<ids::IconID>,
        /// Ancestry name
        pub name: LocalizedString,
        /// Short English description
        pub shortDescription: Option<String>
    }

    pub fn load_ancestries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AncestryID, Ancestry), SDELoadError>>, SDELoadError> {
        load_file::<Ancestry, R>(archive, "ancestries.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.ancestryID, entry))))
    }

    /// Character Bloodline; Character creation element
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Bloodline {
        /// Identifier for this Bloodline, see [`ids::BloodlineID`]
        #[serde(rename="_key")]
        pub bloodlineID: ids::BloodlineID,
        /// Default NPC Corporation for characters with this Bloodline
        pub corporationID: ids::CorporationID,
        /// Bloodline description, as shown in game client
        pub description: LocalizedString,
        /// Icon, if specified
        pub iconID: Option<ids::IconID>,
        /// Bloodline name
        pub name: LocalizedString,
        /// Character race this bloodline is a part of
        pub raceID: ids::RaceID,
        /// Skill training attribute modifier
        pub charisma: i32,
        /// Skill training attribute modifier
        pub intelligence: i32,
        /// Skill training attribute modifier
        pub memory: i32,
        /// Skill training attribute modifier
        pub perception: i32,
        /// Skill training attribute modifier
        pub willpower: i32,
    }

    pub fn load_bloodlines<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::BloodlineID, Bloodline), SDELoadError>>, SDELoadError> {
        load_file::<Bloodline, R>(archive, "bloodlines.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.bloodlineID, entry))))
    }

    /// Industry Blueprint. Also describes Reaction Formulae and the Sleeper Relics used in T3 production
    ///
    /// Note: The SDE provides Blueprint Copy and Blueprint Original data as 'merged' into a single entry for the Blueprint's typeID.
    /// 'Copying' & 'Research Time/Material' activities are not usable with BPCs, 'Invention' activity is not usable with BPOs.
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Blueprint {
        /// Key; Blueprint TypeID. Duplicate of explicit `blueprintTypeID` field in entry. This library current favours using the explicit field, this may change.
        /// In the event this is de-duplicated by removing the entry field this field will be renamed to [`Blueprint::blueprintTypeID`] to retain backwards compatibility.
        #[serde(rename="_key")]
        #[allow(unused)]
        blueprintTypeID_key: ids::TypeID,
        /// TypeID of this blueprint. BP Originals and BP Copies share the same TypeID.
        pub blueprintTypeID: ids::TypeID,
        /// The maximum amount of job runs that can be "printed" on a single blueprint copy
        ///
        /// This is not the limit of repeats that can be scheduled in a single manufacturing/copying/etc job.
        pub maxProductionLimit: i32,
        /// Activities available for this blueprint type
        pub activities: BlueprintActivities
    }

    /// Blueprint activities for a [`Blueprint`]
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct BlueprintActivities {
        /// Blueprint copying activity. When present on blueprint types, only applicable to blueprint *originals*
        pub copying: Option<BPActivity>,
        /// Manufacturing activity
        pub manufacturing: Option<BPActivity>,
        /// Material Efficiency Research activity. When present on blueprint types, only applicable to blueprint *originals*
        pub research_material: Option<BPActivity>,
        /// Time Efficiency Research activity. When present on blueprint types, only applicable to blueprint *originals*
        pub research_time: Option<BPActivity>,
        /// Invention activity. When present on blueprint types, only applicable to blueprint *copies*. Also applicable to Sleeper Relics
        pub invention: Option<BPActivity>,
        /// Reaction activity
        pub reaction: Option<BPActivity>,
    }

    /// Blueprint activity
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct BPActivity {
        /// Materials and quantity required for one run of this activity
        #[serde(deserialize_with="deserialize_activity_materials", default)]
        pub materials: IndexMap<ids::TypeID, u32>,
        /// Products, quantity, and optional probability for one run of this activity.
        /// Only one product type is allowed per run of this activity; When multiple types of products are available, one must be selected by the player when setting up the industry job
        #[serde(deserialize_with="deserialize_activity_products", default)]
        pub products: IndexMap<ids::TypeID, (u32, Option<f64>)>,
        /// Skills required to set up a run of this activity
        #[serde(deserialize_with="deserialize_activity_skills", default)]
        pub skills: IndexMap<ids::TypeID, numbers::SkillLevel>,
        /// Time required for one run of this activity, in seconds
        pub time: u32
    }
    fn deserialize_activity_materials<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<ids::TypeID, u32>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        pub struct BPMaterial {
            typeID: ids::TypeID,
            quantity: u32
        }

        pub struct MaterialVisitor;
        impl<'de> Visitor<'de> for MaterialVisitor {
            type Value = IndexMap<ids::TypeID, u32>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of blueprint materials (typeID & quantity)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<BPMaterial>()? {
                    map.insert(value.typeID, value.quantity);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(MaterialVisitor)
    }
    fn deserialize_activity_products<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<ids::TypeID, (u32, Option<f64>)>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        pub struct BPProduct {
            typeID: ids::TypeID,
            quantity: u32,
            probability: Option<f64>
        }

        pub struct ProductVisitor;
        impl<'de> Visitor<'de> for ProductVisitor {
            type Value = IndexMap<ids::TypeID, (u32, Option<f64>)>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of blueprint products (typeID, quantity & probability)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<BPProduct>()? {
                    map.insert(value.typeID, (value.quantity, value.probability));
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(ProductVisitor)
    }
    fn deserialize_activity_skills<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<ids::TypeID, numbers::SkillLevel>, D::Error> {
        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        pub struct BPSkill {
            typeID: ids::TypeID,
            level: numbers::SkillLevel,
        }

        pub struct SkillVisitor;
        impl<'de> Visitor<'de> for SkillVisitor {
            type Value = IndexMap<ids::TypeID, numbers::SkillLevel>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of blueprint products (typeID, quantity & probability)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<BPSkill>()? {
                    map.insert(value.typeID, value.level);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(SkillVisitor)
    }

    pub fn load_blueprints<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, Blueprint), SDELoadError>>, SDELoadError> {
        load_file::<Blueprint, R>(archive, "blueprints.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.blueprintTypeID, entry))))
    }


    /// Item Type 'Category'; Collection of [Groups](Group)
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Category {
        /// ID for this category
        #[serde(rename="_key")]
        pub categoryID: ids::TypeID,
        /// Name of this category
        pub name: LocalizedString,
        /// 'Published' status; If false, not visible to players in the game client
        pub published: bool,
        /// Icon, if specified
        pub iconID: Option<ids::IconID>
    }

    pub fn load_categories<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CategoryID, Category), SDELoadError>>, SDELoadError> {
        load_file::<Category, R>(archive, "categories.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.categoryID, entry))))
    }

    /// Ship Mastery Certificate
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Certificate {
        /// ID for this certificate
        #[serde(rename="_key")]
        pub certificateID: ids::CertificateID,
        /// Skill [`Group`] this Certificate is for
        pub groupID: ids::GroupID,
        /// Certificate name
        pub name: LocalizedString,
        /// Certificate description
        pub description: LocalizedString,
        /// Ships this certificate is recommended for
        #[serde(default)]
        pub recommendedFor: Vec<ids::TypeID>,
        /// Skill levels for this certificate
        #[serde(rename="skillTypes", deserialize_with="deserialize_inline_entry_map")]
        pub skillLevels: IndexMap<ids::TypeID, CertificateSkillLevels>
    }

    /// Skill levels required for a certificate level
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct CertificateSkillLevels {
        /// Skill this 'levels' data is for
        #[serde(rename="_key")]
        pub skillTypeID: ids::TypeID,
        /// Skill level required for 'basic' certificate
        pub basic: numbers::SkillLevel,
        /// Skill level required for 'standard' certificate
        pub standard: numbers::SkillLevel,
        /// Skill level required for 'improved' certificate
        pub improved: numbers::SkillLevel,
        /// Skill level required for 'advanced' certificate
        pub advanced: numbers::SkillLevel,
        /// Skill level required for 'elite' certificate
        pub elite: numbers::SkillLevel,
    }

    impl InlineEntry<ids::TypeID> for CertificateSkillLevels {
        fn key(&self) -> ids::TypeID {
            self.skillTypeID
        }
    }

    pub fn load_certificates<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CertificateID, Certificate), SDELoadError>>, SDELoadError> {
        load_file::<Certificate, R>(archive, "certificates.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.certificateID, entry))))
    }

    /// Character skill training Attribute
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CharacterAttribute {
        /// ID for this character attribute
        #[serde(rename="_key")]
        pub characterAttributeID: ids::CharacterAttributeID,
        /// Name
        pub name: LocalizedString,
        /// Description, in English
        pub description: String,
        /// Icon for attribute
        pub iconID: ids::IconID,
        /// Notes, in English
        pub notes: String,
        /// Short description, in English
        pub shortDescription: String
    }

    pub fn load_character_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterAttributeID, CharacterAttribute), SDELoadError>>, SDELoadError> {
        load_file::<CharacterAttribute, R>(archive, "characterAttributes.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.characterAttributeID, entry))))
    }
    /// Contraband status information for a [`Type`]
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ContrabandType {
        /// Type for which this Contraband information applies
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        /// Per-faction contraband info; An entry means the Type is contraband in the given faction
        #[serde(deserialize_with="deserialize_inline_entry_map")]
        pub factions: IndexMap<ids::FactionID, ContrabandFactionInfo>
    }

    /// Per-faction Contraband information
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ContrabandFactionInfo {
        /// Faction for which this info applies
        #[serde(rename="_key")]
        pub factionID: ids::FactionID,
        /// Minimum solarsystem security in which NPC customs agents will attack if this type of contraband is carried
        ///
        /// Mechanic appears to be disabled, with this value always set greater than the maximum security level of 1.0
        pub attackMinSec: f64,
        /// Minimum solarsystem security in which NPC customs confiscate this type of contraband
        pub confiscateMinSec: f64,
        /// Fine (multiplier * item value carried, e.g. 4.5 = 450% of the contraband's value) to be paid if caught
        pub fineByValue: f64,
        /// Faction standing loss if caught carrying this contraband
        pub standingLoss: f64
    }

    impl InlineEntry<ids::FactionID> for ContrabandFactionInfo {
        fn key(&self) -> ids::FactionID {
            self.factionID
        }
    }

    pub fn load_contraband_types<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, ContrabandType), SDELoadError>>, SDELoadError> {
        load_file::<ContrabandType, R>(archive, "contrabandTypes.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    /// Resources required for Player-owned-Starbase Control Tower operation
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ControlTowerResources {
        /// TypeID of the Control Tower type this information applies to
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        /// Resources required for the operation of this Control Tower
        pub resources: Vec<ControlTowerResourceInfo>
    }

    /// Resources required for Player-owned-Starbase Control Tower operation
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ControlTowerResourceInfo {
        /// Purpose for which this resource is required. (Either Online operation or Reinforcement)
        pub purpose: ResourcePurpose,
        /// Quantity required per hour of operation
        pub quantity: u32,
        /// Type of the required resource
        pub resourceTypeID: ids::TypeID,
        /// If set, this resource is only required if operating in the Faction's space
        pub factionID: Option<ids::FactionID>,
        /// If set, this resource is only required if operating above the specified security level
        pub minSecurityLevel: Option<f64>
    }

    #[repr(u32)]
    #[derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr, Debug)]
    pub enum ResourcePurpose {
        /// Resource required for keeping a Control Tower online
        Online = 1,
        /// Resource required for Control Tower reinforcement
        Reinforce = 4
    }

    pub fn load_controltower_resources<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, ControlTowerResources), SDELoadError>>, SDELoadError> {
        load_file::<ControlTowerResources, R>(archive, "controlTowerResources.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    /// NPC Station Activity/"Specialization"
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CorporationActivity {
        /// ID for this activity
        #[serde(rename="_key")]
        pub corporationActivityID: ids::CorporationActivityID,
        /// Name for this activity
        pub name: LocalizedString
    }

    pub fn load_corporation_activities<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CorporationActivityID, CorporationActivity), SDELoadError>>, SDELoadError> {
        load_file::<CorporationActivity, R>(archive, "corporationActivities.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.corporationActivityID, entry))))
    }

    /// 'Warefare Buff'; Command Burst bonus effects
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct WarfareBuff {
        /// ID for this warfare buff. Referenced by attributes on Command Burst charges
        #[serde(rename="_key")]
        pub warfareBuffID: ids::WarfareBuffID,
        /// Aggregate mode for multiple buffs; Whether the maximum or minimum value is selected when multiple buffs of different strengths are applied to a ship
        pub aggregateMode: WarfareBuffAggregateMode,
        /// Developer description, in English
        pub developerDescription: String,
        /// Display name, as shown in tooltip in-game
        pub displayName: Option<LocalizedString>,
        /// Attributes whose effects are applied as Item Modifiers
        #[serde(default)]
        #[serde(deserialize_with="deserialize_warfarebuff_item_modifiers")]
        pub itemModifiers: Vec<ids::AttributeID>,
        /// Attributes whose effects are applied as Location Group Modifiers
        #[serde(default)]
        pub locationGroupModifiers: Vec<WarfareBuffLocationGroupModifier>,
        /// Attributes whose effects are applied as Location Modifiers
        #[serde(default)]
        #[serde(deserialize_with="deserialize_warfarebuff_location_modifiers")]
        pub locationModifiers: Vec<ids::AttributeID>,
        /// Attributes whose effects are applied as Location with-required-skill Modifiers
        #[serde(default)]
        pub locationRequiredSkillModifiers: Vec<WarfareBuffLocationRequiredSkillModifier>,
        /// Operation applied by modifiers
        pub operationName: WarfareBuffOperation,
        /// How the effect value is displayed in-game
        pub showOutputValueInUI: WarfareBuffUIMode
    }

    fn deserialize_warfarebuff_item_modifiers<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ids::AttributeID>, D::Error> {
        struct SeqVisitor;
        impl<'de> Visitor<'de> for SeqVisitor {
            type Value = Vec<ids::AttributeID>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of warfarebuff item modifier attributes")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                struct WarfareBuffItemModifier {
                    pub dogmaAttributeID: ids::AttributeID
                }

                let size_hint = seq.size_hint();
                let mut vec = size_hint.map(Vec::with_capacity).unwrap_or_else(Vec::new);
                while let Some(value) = seq.next_element::<WarfareBuffItemModifier>()? {
                    vec.push(value.dogmaAttributeID)
                }
                Ok(vec)
            }
        }

        deserializer.deserialize_seq(SeqVisitor)
    }

    fn deserialize_warfarebuff_location_modifiers<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ids::AttributeID>, D::Error> {
        struct SeqVisitor;
        impl<'de> Visitor<'de> for SeqVisitor {
            type Value = Vec<ids::AttributeID>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of warfarebuff location modifier attributes")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                struct WarfareBuffLocationModifier {
                    dogmaAttributeID: ids::AttributeID
                }

                let size_hint = seq.size_hint();
                let mut vec = size_hint.map(Vec::with_capacity).unwrap_or_else(Vec::new);
                while let Some(value) = seq.next_element::<WarfareBuffLocationModifier>()? {
                    vec.push(value.dogmaAttributeID)
                }
                Ok(vec)
            }
        }

        deserializer.deserialize_seq(SeqVisitor)
    }

    /// Aggregate mode for warfare buff effect stacking
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub enum WarfareBuffAggregateMode {
        /// If multiple buffs stack, the maximum value is selected
        Maximum,
        /// If multiple buffs stack, the minimum value is selected
        Minimum
    }

    /// Dogma operation for warfare buff
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub enum WarfareBuffOperation {
        // Dogma is weird and complicated, so no individual docs on these
        PostMul, PostPercent, ModAdd, PostAssignment
    }

    /// Warfare buff display mode
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub enum WarfareBuffUIMode {
        /// Buff amount is not shown
        Hide,
        /// Buff amount is shown as-is, e.g. `10 -> "10%", -10 -> "-10%"`
        ShowNormal,
        /// Buff amount is shown inverted, e.g. `10 -> "-10%", -10 -> "10%"`
        ShowInverted
    }

    /// Attribute whose effects are applied as Location Group Modifier
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct WarfareBuffLocationGroupModifier {
        /// Attribute source for effect
        pub dogmaAttributeID: ids::AttributeID,
        /// Applicable group
        pub groupID: ids::GroupID
    }

    /// Attributes whose effects are applied as Location with-required-skill Modifiers
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct WarfareBuffLocationRequiredSkillModifier {
        /// Attribute source for effect
        pub dogmaAttributeID: ids::AttributeID,
        /// Skill required by applicable types
        pub skillID: ids::TypeID
    }

    pub fn load_dbuff_collections<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::WarfareBuffID, WarfareBuff), SDELoadError>>, SDELoadError> {
        load_file::<WarfareBuff, R>(archive, "dbuffCollections.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.warfareBuffID, entry))))
    }

    /// Attribute Category, grouping of [`Attribute`]
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AttributeCategory {
        /// ID for this category
        #[serde(rename="_key")]
        pub attributeCategoryID: ids::AttributeCategoryID,
        /// Category name, in English
        pub name: String,
        /// Description, in English
        pub description: Option<String>
    }

    pub fn load_dogma_attribute_categories<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AttributeCategoryID, AttributeCategory), SDELoadError>>, SDELoadError> {
        load_file::<AttributeCategory, R>(archive, "dogmaAttributeCategories.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.attributeCategoryID, entry))))
    }

    /// Dogma Attribute, describing properties for [`Type`]s. Such as HP, maximum velocity, and other item stats
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Attribute {
        /// ID for this attribute
        #[serde(rename="_key")]
        pub attributeID: ids::AttributeID,
        /// (Optional) [`AttributeCategory`] for this attribute
        pub attributeCategoryID: Option<ids::AttributeCategoryID>,
        /// Unknown
        pub chargeRechargeTimeID: Option<u32>,
        /// Unknown
        pub dataType: i32,
        /// Default implied value if an attribute is not explicitly given for a type
        pub defaultValue: f64,
        /// "Developer" description in English
        pub description: Option<String>,
        /// In-game name, as displayed in item stats
        pub displayName: Option<LocalizedString>,
        /// If true, display this attribute when it's value is `0.0`. If false, attribute is hidden when the value is `0.0`
        pub displayWhenZero: bool,
        /// If set to true, higher values are considered better than lower values. Inverted for false. Used for e.g. determining which module have a better attribute value than another module
        pub highIsGood: bool,
        /// Icon for attribute, as disabled in in-game stats
        pub iconID: Option<ids::IconID>,
        /// Attribute specifying the maximum value for this attribute
        pub maxAttributeID: Option<ids::AttributeID>,
        /// Attribute specifying the minimum value for this attribute
        pub minAttributeID: Option<ids::AttributeID>,
        /// "Developer" name in English
        pub name: String,
        /// 'Published' status; If false, not visible to players in the game client
        pub published: bool,
        /// If false, this attribute is subject to stacking penalties /* TODO: DOC LINK */
        pub stackable: bool,
        /// Tooltip tile, as displayed when hovering over an attribute in-game
        pub tooltipTitle: Option<LocalizedString>,
        /// Tooltip description, as displayed when hovering over an attribute in-game
        pub tooltipDescription: Option<LocalizedString>,
        /// [`Unit`] for this attribute's values    /* TODO: Proper link to Unit */
        pub unitID: Option<ids::UnitID>,
    }

    pub fn load_dogma_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AttributeID, Attribute), SDELoadError>>, SDELoadError> {
        load_file::<Attribute, R>(archive, "dogmaAttributes.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.attributeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Effect {
        #[serde(rename="_key")]
        pub effectID: ids::EffectID,
        pub description: Option<LocalizedString>,
        pub disallowAutoRepeat: bool,
        pub dischargeAttributeID: Option<ids::AttributeID>,
        pub displayName: Option<LocalizedString>,
        pub distribution: Option<i32>,  // TODO: Figure out what this is for
        pub durationAttributeID: Option<ids::AttributeID>,
        pub effectCategoryID: ids::EffectCategoryID,
        pub electronicChance: bool,
        pub falloffAttributeID: Option<ids::AttributeID>,
        pub fittingUsageChanceAttributeID: Option<ids::AttributeID>,
        pub guid: Option<String>,
        pub iconID: Option<ids::IconID>,
        pub isAssistance: bool,
        pub isOffensive: bool,
        pub isWarpSafe: bool,
        #[serde(default)]
        pub modifierInfo: Vec<ModifierInfo>,
        pub name: String,
        pub npcActivationChanceAttributeID: Option<ids::AttributeID>,
        pub npcUsageChanceAttributeID: Option<ids::AttributeID>,
        pub propulsionChance: bool,
        pub published: bool,
        pub rangeAttributeID: Option<ids::AttributeID>,
        pub rangeChance: bool,
        pub resistanceAttributeID: Option<ids::AttributeID>,
        pub sfxName: Option<String>,    // TODO: Always the string "None" if present?
        pub trackingSpeedAttributeID: Option<ids::AttributeID>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct ModifierInfo {
        pub domain: String,
        pub func: String,
        pub operation: Option<i32>,
        pub modifiedAttributeID: Option<ids::AttributeID>,
        pub modifyingAttributeID: Option<ids::AttributeID>,
        pub groupID: Option<ids::GroupID>,
        pub effectID: Option<ids::EffectID>,
        pub skillTypeID: Option<ids::TypeID>
    }

    pub fn load_dogma_effects<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::EffectID, Effect), SDELoadError>>, SDELoadError> {
        load_file::<Effect, R>(archive, "dogmaEffects.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.effectID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct DogmaUnit {
        #[serde(rename="_key")]
        pub unitID: ids::UnitID,    // TODO: Merge unitID with util::unit::Unit
        pub description: Option<LocalizedString>,
        pub displayName: Option<LocalizedString>,
        pub name: String,
    }

    pub fn load_dogma_units<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::UnitID, DogmaUnit), SDELoadError>>, SDELoadError> {
        load_file::<DogmaUnit, R>(archive, "dogmaUnits.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.unitID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct DynamicItemAttributes {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        #[serde(deserialize_with="deserialize_inline_entry_map")]
        pub attributeIDs: IndexMap<ids::AttributeID, DynamicAttributeInfo>,
        pub inputOutputMapping: Vec<DynamicItemAttributesIOMapping>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct DynamicAttributeInfo {
        #[serde(rename="_key")]
        pub attributeID: ids::AttributeID,
        pub highIsGood: Option<bool>,
        pub max: f64,
        pub min: f64
    }

    impl InlineEntry<ids::AttributeID> for DynamicAttributeInfo {
        fn key(&self) -> ids::AttributeID {
            self.attributeID
        }
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct DynamicItemAttributesIOMapping {
        pub applicableTypes: Vec<ids::TypeID>,
        pub resultingType: ids::TypeID
    }

    pub fn load_dynamic_item_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, DynamicItemAttributes), SDELoadError>>, SDELoadError> {
        load_file::<DynamicItemAttributes, R>(archive, "dynamicItemAttributes.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Faction {
        #[serde(rename="_key")]
        pub factionID: ids::FactionID,
        pub corporationID: Option<ids::CorporationID>,
        pub description: LocalizedString,
        pub flatLogo: Option<String>,
        pub flatLogoWithName: Option<String>,
        pub iconID: ids::IconID,
        pub memberRaces: Vec<ids::RaceID>,
        pub militiaCorporationID: Option<ids::CorporationID>,
        pub name: LocalizedString,
        pub shortDescription: Option<LocalizedString>,
        pub sizeFactor: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub uniqueName: bool
    }

    pub fn load_factions<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::FactionID, Faction), SDELoadError>>, SDELoadError> {
        load_file::<Faction, R>(archive, "factions.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.factionID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Graphic {
        #[serde(rename="_key")]
        pub graphicID: ids::GraphicID,
        pub graphicFile: Option<String>,
        pub iconFolder: Option<String>,
        pub sofFactionName: Option<String>,
        pub sofHullName: Option<String>,
        #[serde(default)]
        pub sofLayout: Vec<String>,
        pub sofMaterialSetID: Option<ids::MaterialSetID>,
        pub sofRaceName: Option<String>,
    }

    pub fn load_graphics<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::GraphicID, Graphic), SDELoadError>>, SDELoadError> {
        load_file::<Graphic, R>(archive, "graphics.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.graphicID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Group {
        #[serde(rename="_key")]
        pub groupID: ids::GroupID,
        pub anchorable: bool,
        pub anchored: bool,
        pub categoryID: ids::CategoryID,
        pub fittableNonSingleton: bool,
        pub iconID: Option<ids::IconID>,
        pub name: LocalizedString,
        pub published: bool,
        pub useBasePrice: bool,
    }

    pub fn load_groups<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::GroupID, Group), SDELoadError>>, SDELoadError> {
        load_file::<Group, R>(archive, "groups.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.groupID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Icon {
        #[serde(rename="_key")]
        pub iconID: ids::IconID,
        pub iconFile: String
    }

    pub fn load_icons<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::IconID, Icon), SDELoadError>>, SDELoadError> {
        load_file::<Icon, R>(archive, "icons.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.iconID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Landmark {
        #[serde(rename="_key")]
        pub landmarkID: ids::LandmarkID,
        pub description: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub locationID: Option<ids::LocationID>,
        pub name: LocalizedString,
        pub position: Position
    }

    pub fn load_landmarks<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::LandmarkID, Landmark), SDELoadError>>, SDELoadError> {
        load_file::<Landmark, R>(archive, "landmarks.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.landmarkID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AsteroidBelt {
        #[serde(rename="_key")]
        pub asteroidBeltID: ids::AsteroidBeltID,
        pub celestialIndex: u32,
        pub orbitID: ids::ItemID,
        pub orbitIndex: u32,
        pub position: Position,
        pub radius: Option<f64>,
        pub solarSystemID: ids::SolarSystemID,
        pub statistics: Option<AsteroidBeltStatistics>,
        pub typeID: ids::TypeID,
        pub uniqueName: Option<LocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct AsteroidBeltStatistics {
        pub density: f64,
        pub eccentricity: f64,
        pub escapeVelocity: f64,
        pub locked: bool,
        pub massGas: Option<f64>,
        pub massDust: f64,
        pub orbitPeriod: f64,
        pub orbitRadius: f64,
        pub rotationRate: f64,
        pub spectralClass: String,
        pub surfaceGravity: f64,
        pub temperature: f64
    }

    pub fn load_asteroid_belts<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AsteroidBeltID, AsteroidBelt), SDELoadError>>, SDELoadError> {
        load_file::<AsteroidBelt, R>(archive, "mapAsteroidBelts.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.asteroidBeltID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Constellation {
        #[serde(rename="_key")]
        pub constellationID: ids::ConstellationID,
        pub regionID: ids::RegionID,
        pub factionID: Option<ids::FactionID>,
        pub position: Position,
        pub name: LocalizedString,
        pub solarSystemIDs: Vec<ids::SolarSystemID>,
        pub wormholeClassID: Option<ids::WormholeClassID>
    }

    pub fn load_constellations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::ConstellationID, Constellation), SDELoadError>>, SDELoadError> {
        load_file::<Constellation, R>(archive, "mapConstellations.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.constellationID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Moon {
        #[serde(rename="_key")]
        pub moonID: ids::MoonID,
        pub attributes: MoonAttributes,
        pub celestialIndex: u32,
        #[serde(default)]
        pub npcStationIDs: Vec<ids::StationID>,
        pub orbitID: ids::ItemID,
        pub orbitIndex: u32,
        pub position: Position,
        pub radius: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub statistics: Option<MoonStatistics>,
        pub typeID: ids::TypeID,
        pub uniqueName: Option<LocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MoonStatistics {
        pub density: f64,
        pub eccentricity: f64,
        pub escapeVelocity: f64,
        pub locked: bool,
        pub massDust: f64,
        pub massGas: Option<f64>,
        pub pressure: f64,
        pub orbitPeriod: f64,
        pub orbitRadius: f64,
        pub rotationRate: f64,
        pub spectralClass: String,
        pub surfaceGravity: f64,
        pub temperature: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MoonAttributes {
        pub heightMap1: u32,
        pub heightMap2: u32,
        pub shaderPreset: u32
    }

    pub fn load_moons<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MoonID, Moon), SDELoadError>>, SDELoadError> {
        load_file::<Moon, R>(archive, "mapMoons.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.moonID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Planet {
        #[serde(rename="_key")]
        pub planetID: ids::PlanetID,
        #[serde(default)]
        pub asteroidBeltIDs: Vec<ids::AsteroidBeltID>,
        pub attributes: PlanetAttributes,
        pub celestialIndex: u32,
        #[serde(default)]
        pub moonIDs: Vec<ids::MoonID>,
        #[serde(default)]
        pub npcStationIDs: Vec<ids::StationID>,
        pub orbitID: Option<ids::ItemID>,
        pub position: Position,
        pub radius: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub statistics: PlanetStatistics,
        pub typeID: ids::TypeID,
        pub uniqueName: Option<LocalizedString>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetStatistics {
        pub density: f64,
        pub eccentricity: f64,
        pub escapeVelocity: f64,
        pub locked: bool,
        pub massDust: f64,
        pub massGas: Option<f64>,
        pub pressure: f64,
        pub orbitPeriod: Option<f64>,
        pub orbitRadius: Option<f64>,
        pub rotationRate: f64,
        pub spectralClass: String,
        pub surfaceGravity: Option<f64>,
        pub temperature: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetAttributes {
        pub heightMap1: u32,
        pub heightMap2: u32,
        pub population: bool,
        pub shaderPreset: u32
    }

    pub fn load_planets<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetID, Planet), SDELoadError>>, SDELoadError> {
        load_file::<Planet, R>(archive, "mapPlanets.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.planetID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Region {
        #[serde(rename="_key")]
        pub regionID: ids::RegionID,
        pub constellationIDs: Vec<ids::ConstellationID>,
        pub description: Option<LocalizedString>,
        pub factionID: Option<ids::FactionID>,
        pub name: LocalizedString,
        pub nebulaID: u32,    // TODO: Assign ID type
        pub position: Position,
        pub wormholeClassID: Option<ids::WormholeClassID>
    }

    pub fn load_regions<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::RegionID, Region), SDELoadError>>, SDELoadError> {
        load_file::<Region, R>(archive, "mapRegions.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.regionID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SolarSystem {
        #[serde(rename="_key")]
        pub solarSystemID: ids::SolarSystemID,
        pub border: Option<bool>,
        pub constellationID: ids::ConstellationID,
        pub corridor: Option<bool>,
        #[serde(default)]
        pub disallowedAnchorCategories: Vec<ids::CategoryID>,
        #[serde(default)]
        pub disallowedAnchorGroups: Vec<ids::GroupID>,
        pub factionID: Option<ids::FactionID>,
        pub fringe: Option<bool>,
        pub hub: Option<bool>,
        pub international: Option<bool>,
        pub luminosity: Option<f64>,
        pub name: LocalizedString,
        #[serde(default)]
        pub planetIDs: Vec<ids::PlanetID>,
        pub position: Position,
        pub position2D: Option<Position2D>,
        pub radius: f64,
        pub regionID: ids::RegionID,
        pub regional: Option<bool>,
        // pub secondarySun: Option<SecondarySun>, Removed T.T CCPls; TODO: Add doc comment on type pointing to hardcoded data entry for this
        pub securityClass: Option<String>,
        pub securityStatus: f64,
        pub starID: Option<ids::StarID>,
        #[serde(default)]
        pub stargateIDs: Vec<ids::StargateID>,
        pub visualEffect: Option<String>,
        pub wormholeClassID: Option<ids::WormholeClassID>,
    }

    pub fn load_solarsystems<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::SolarSystemID, SolarSystem), SDELoadError>>, SDELoadError> {
        load_file::<SolarSystem, R>(archive, "mapSolarSystems.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.solarSystemID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Stargate {
        #[serde(rename="_key")]
        pub stargateID: ids::StargateID,
        pub destination: StargateDestination,
        pub position: Position,
        pub solarSystemID: ids::SolarSystemID,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StargateDestination {
        pub solarSystemID: ids::SolarSystemID,
        pub stargateID: ids::StargateID
    }

    pub fn load_stargates<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StargateID, Stargate), SDELoadError>>, SDELoadError> {
        load_file::<Stargate, R>(archive, "mapStargates.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.stargateID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Star {
        #[serde(rename="_key")]
        pub starID: ids::StarID,
        pub radius: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub statistics: StarStatistics,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StarStatistics {
        pub age: f64,
        pub life: f64,
        pub luminosity: f64,
        pub spectralClass: String,
        pub temperature: f64
    }

    pub fn load_stars<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StarID, Star), SDELoadError>>, SDELoadError> {
        load_file::<Star, R>(archive, "mapStars.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.starID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MarketGroup {
        #[serde(rename="_key")]
        pub marketGroupID: ids::MarketGroupID,
        pub description: Option<LocalizedString>,
        pub hasTypes: bool,
        pub iconID: Option<ids::IconID>,
        pub name: LocalizedString,
        pub parentGroupID: Option<ids::MarketGroupID>
    }

    pub fn load_market_groups<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MarketGroupID, MarketGroup), SDELoadError>>, SDELoadError> {
        load_file::<MarketGroup, R>(archive, "marketGroups.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.marketGroupID, entry))))
    }

    #[derive(Debug)]
    #[allow(non_snake_case)]
    pub struct MasteryLevels {
        pub lvl1: Vec<ids::CertificateID>,
        pub lvl2: Vec<ids::CertificateID>,
        pub lvl3: Vec<ids::CertificateID>,
        pub lvl4: Vec<ids::CertificateID>,
        pub lvl5: Vec<ids::CertificateID>
    }

    impl<'de> Deserialize<'de> for MasteryLevels {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
            struct MasteryVisitor;
            impl<'de> Visitor<'de> for MasteryVisitor {
                type Value = MasteryLevels;

                fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                    formatter.write_str("array of mastery levels")
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                    use serde::de::Error;
                    let mut levels = MasteryLevels {
                        lvl1: Vec::new(),
                        lvl2: Vec::new(),
                        lvl3: Vec::new(),
                        lvl4: Vec::new(),
                        lvl5: Vec::new(),
                    };

                    while let Some(ExplicitMapEntry { _key, _value }) = seq.next_element::<ExplicitMapEntry<u8, Vec<ids::CertificateID>>>()? {
                        match _key {
                            0 => levels.lvl1 = _value,
                            1 => levels.lvl2 = _value,
                            2 => levels.lvl3 = _value,
                            3 => levels.lvl4 = _value,
                            4 => levels.lvl5 = _value,
                            _ => return Err(A::Error::invalid_value(Unexpected::Other("mastery level"), &"mastery level in range 0..=4"))
                        }
                    }
                    Ok(levels)
                }
            }

            deserializer.deserialize_seq(MasteryVisitor)
        }
    }

    pub fn load_masteries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, MasteryLevels), SDELoadError>>, SDELoadError> {
        load_file::<ExplicitMapEntry<_, _>, R>(archive, "masteries.jsonl")
            .map(|iter| iter.map(|value| value.map(|entry| (entry._key, entry._value))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MetaGroup {
        #[serde(rename="_key")]
        pub metaGroupID: ids::MetaGroupID,
        pub color: Option<MetaGroupColor>,
        pub name: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub iconSuffix: Option<String>,
        pub description: Option<LocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct MetaGroupColor {
        pub r: f64,
        pub g: f64,
        pub b: f64,
    }

    pub fn load_meta_groups<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MetaGroupID, MetaGroup), SDELoadError>>, SDELoadError> {
        load_file::<MetaGroup, R>(archive, "metaGroups.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.metaGroupID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCharacter {
        #[serde(rename="_key")]
        pub characterID: ids::CharacterID,
        pub agent: Option<NpcCharacterAgent>,
        pub ancestryID: Option<ids::AncestryID>,
        pub bloodlineID: ids::BloodlineID,
        pub careerID: Option<ids::CareerID>,
        pub ceo: bool,
        pub corporationID: ids::CorporationID,
        pub description: Option<String>,
        pub gender: bool,
        pub locationID: Option<ids::LocationID>,
        pub name: LocalizedString,
        pub raceID: ids::RaceID,
        pub schoolID: Option<ids::SchoolID>,
        #[serde(default)]
        pub skills: Vec<NpcCharacterSkill>,
        pub specialityID: Option<ids::SpecialtyID>,
        pub startDate: Option<String>,
        pub uniqueName: bool
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCharacterSkill {
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCharacterAgent {
        pub agentTypeID: ids::TypeID,
        pub divisionID: ids::DivisionID,
        pub isLocator: bool,
        pub level: i32,
    }

    pub fn load_npc_characters<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterID, NpcCharacter), SDELoadError>>, SDELoadError> {
        load_file::<NpcCharacter, R>(archive, "npcCharacters.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.characterID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCorporationDivision {
        #[serde(rename="_key")]
        pub divisionID: ids::DivisionID,
        pub description: Option<LocalizedString>,
        pub displayName: Option<String>,
        pub internalName: String,
        pub leaderTypeName: LocalizedString,
        pub name: LocalizedString
    }

    pub fn load_npc_corporation_divisions<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::DivisionID, NpcCorporationDivision), SDELoadError>>, SDELoadError> {
        load_file::<NpcCorporationDivision, R>(archive, "npcCorporationDivisions.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.divisionID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcCorporation {
        #[serde(rename="_key")]
        pub corporationID: ids::CorporationID,
        pub allowedMemberRaces: Option<Vec<ids::RaceID>>,
        pub ceoID: Option<ids::CharacterID>,
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub corporationTrades: IndexMap<ids::TypeID, f64>,
        pub deleted: bool,
        pub description: Option<LocalizedString>,
        #[serde(default, deserialize_with="deserialize_inline_entry_map")]
        pub divisions: IndexMap<ids::DivisionID, CorporationDivision>,
        pub enemyID: Option<ids::CorporationID>,
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub exchangeRates: IndexMap<ids::CorporationID, f64>,
        pub extent: String, // TODO: Enum
        pub factionID: Option<ids::FactionID>,
        pub friendID: Option<ids::CorporationID>,
        pub hasPlayerPersonnelManager: bool,
        pub iconID: Option<ids::IconID>,
        pub initialPrice: f64,
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub investors: IndexMap<ids::CorporationID, i32>,
        #[serde(default)]
        pub lpOfferTables: Vec<u32>,    // TODO: Assign ID type
        pub mainActivityID: Option<i32>,    // TODO: Assign ID type, possibly station activity ID?
        pub memberLimit: i32,
        pub minSecurity: f64,
        pub minimumJoinStanding: f64,
        pub name: LocalizedString,
        pub raceID: Option<ids::RaceID>,
        pub secondaryActivityID: Option<i32>,    // TODO: Assign ID type, possibly station activity ID?
        pub sendCharTerminationMessage: bool,
        pub shares: u64,
        pub size: String,   // TODO: Enum
        pub sizeFactor: Option<f64>,
        pub solarSystemID: Option<ids::SolarSystemID>,
        pub stationID: Option<ids::StationID>,
        pub taxRate: f64,
        pub tickerName: String,
        pub uniqueName: bool
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CorporationDivision {
        #[serde(rename="_key")]
        pub divisionID: ids::DivisionID,
        pub divisionNumber: i32,
        pub leaderID: ids::CharacterID,
        pub size: i32
    }

    impl InlineEntry<ids::DivisionID> for CorporationDivision {
        fn key(&self) -> ids::DivisionID {
            self.divisionID
        }
    }

    pub fn load_npc_corporations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CorporationID, NpcCorporation), SDELoadError>>, SDELoadError> {
        load_file::<NpcCorporation, R>(archive, "npcCorporations.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.corporationID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct NpcStation {
        #[serde(rename="_key")]
        pub stationID: ids::StationID,
        pub celestialIndex: Option<u32>,
        pub operationID: ids::StationOperationID,
        pub orbitID: ids::ItemID,
        pub orbitIndex: Option<u32>,
        pub ownerID: ids::CorporationID,
        pub position: Position,
        pub reprocessingEfficiency: f64,
        pub reprocessingHangarFlag: i32,
        pub reprocessingStationsTake: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub typeID: ids::TypeID,
        pub useOperationName: bool
    }

    pub fn load_npc_stations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StationID, NpcStation), SDELoadError>>, SDELoadError> {
        load_file::<NpcStation, R>(archive, "npcStations.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.stationID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetResource {
        #[serde(rename="_key")]
        pub planet_id: ids::PlanetID,
        pub power: Option<i32>,
        pub workforce: Option<i32>,
        pub reagent: Option<PlanetReagent>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetReagent {
        pub amount_per_cycle: i32,
        pub cycle_period: i32,  // Seconds
        pub secured_capacity: f64,  // TODO: Are these floats or i64?
        pub unsecured_capacity: f64,
        pub type_id: ids::TypeID
    }

    pub fn load_planet_resources<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetID, PlanetResource), SDELoadError>>, SDELoadError> {
        load_file::<PlanetResource, R>(archive, "planetResources.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.planet_id, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetSchematic {
        #[serde(rename="_key")]
        pub schematicID: ids::PlanetSchematicID,
        pub cycleTime: u32,
        pub name: LocalizedString,
        pub pins: Vec<ids::TypeID>,
        #[serde(default, deserialize_with="deserialize_inline_entry_map")]
        pub types: IndexMap<ids::TypeID, PlanetSchematicType>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct PlanetSchematicType {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        pub isInput: bool,
        pub quantity: u32
    }

    impl InlineEntry<ids::TypeID> for PlanetSchematicType {
        fn key(&self) -> ids::TypeID {
            self.typeID
        }
    }

    pub fn load_planet_schematics<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetSchematicID, PlanetSchematic), SDELoadError>>, SDELoadError> {
        load_file::<PlanetSchematic, R>(archive, "planetSchematics.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.schematicID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct CharacterRace {
        #[serde(rename="_key")]
        pub raceID: ids::RaceID,
        pub name: LocalizedString,
        pub description: Option<LocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub shipTypeID: Option<ids::TypeID>, // Corvette/"Rookie ship"
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub skills: IndexMap<ids::TypeID, numbers::SkillLevel>
    }

    pub fn load_races<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::RaceID, CharacterRace), SDELoadError>>, SDELoadError> {
        load_file::<CharacterRace, R>(archive, "races.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.raceID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SkinLicense {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        pub duration: i32,
        pub licenseTypeID: ids::TypeID,
        pub skinID: ids::SkinID,
        pub isSingleUse: Option<bool>
    }

    pub fn load_skin_licenses<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, SkinLicense), SDELoadError>>, SDELoadError> {
        load_file::<SkinLicense, R>(archive, "skinLicenses.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SkinMaterial {
        #[serde(rename="_key")]
        pub materialID: ids::SkinMaterialID,
        pub displayName: Option<LocalizedString>,
        pub materialSetID: ids::MaterialSetID,
    }

    pub fn load_skin_materials<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::SkinMaterialID, SkinMaterial), SDELoadError>>, SDELoadError> {
        load_file::<SkinMaterial, R>(archive, "skinMaterials.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.materialID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Skin {
        #[serde(rename="_key")]
        pub skinID: ids::SkinID,
        pub allowCCPDevs: bool,
        pub internalName: String,
        pub skinMaterialID: ids::SkinMaterialID,
        pub types: Vec<ids::TypeID>,
        pub visibleSerenity: bool,
        pub visibleTranquility: bool,
        pub isStructureSkin: Option<bool>,
        pub skinDescription: Option<LocalizedString>
    }

    pub fn load_skins<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::SkinID, Skin), SDELoadError>>, SDELoadError> {
        load_file::<Skin, R>(archive, "skins.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.skinID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SovereigntyUpgrade {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        pub mutually_exclusive_group: String,
        pub power_allocation: Option<i32>,
        pub power_production: Option<i32>,
        pub workforce_allocation: Option<i32>,
        pub workforce_production: Option<i32>,
        pub fuel: Option<SovereigntyUpgradeFuel>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct SovereigntyUpgradeFuel {
        pub type_id: ids::TypeID,
        pub startup_cost: i32,
        pub hourly_upkeep: i32
    }

    pub fn load_sovereignty_upgrades<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, SovereigntyUpgrade), SDELoadError>>, SDELoadError> {
        load_file::<SovereigntyUpgrade, R>(archive, "sovereigntyUpgrades.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StationOperation {
        #[serde(rename="_key")]
        pub operationID: ids::StationOperationID,
        pub activityID: ids::CorporationActivityID,
        pub border: f64,
        pub corridor: f64,
        pub fringe: f64,
        pub hub: f64,
        pub operationName: LocalizedString,
        pub description: Option<LocalizedString>,
        pub ratio: f64,
        pub manufacturingFactor: f64,
        pub researchFactor: f64,
        pub services: Vec<ids::StationServiceID>,
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub stationTypes: IndexMap<u32, ids::TypeID>,    // TODO: Figure out key value
    }

    pub fn load_station_operations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StationOperationID, StationOperation), SDELoadError>>, SDELoadError> {
        load_file::<StationOperation, R>(archive, "stationOperations.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.operationID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct StationService {
        #[serde(rename="_key")]
        pub serviceID: ids::StationServiceID,
        pub serviceName: LocalizedString,
        pub description: Option<LocalizedString>,
    }

    pub fn load_station_services<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StationServiceID, StationService), SDELoadError>>, SDELoadError> {
        load_file::<StationService, R>(archive, "stationServices.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.serviceID, entry))))
    }

    #[derive(Debug, Deserialize, Hash, Eq, PartialEq)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TranslationLanguage {
        #[serde(rename="_key")]
        pub shortName: String,
        pub name: String
    }

    pub fn load_translation_languages<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<TranslationLanguage, SDELoadError>>, SDELoadError> {
        load_file::<_, R>(archive, "translationLanguages.jsonl")
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeBonuses {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        pub iconID: Option<ids::IconID>,
        #[serde(default)]
        pub miscBonuses: Vec<TypeBonus>,
        #[serde(default)]
        pub roleBonuses: Vec<TypeBonus>,
        #[serde(default, rename = "types", deserialize_with="deserialize_explicit_entry_map")]
        pub skillBonuses: IndexMap<ids::TypeID, Vec<TypeBonus>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeBonus {
        pub bonusText: LocalizedString,
        pub importance: i32,
        pub bonus: Option<f64>,
        pub unitID: Option<ids::UnitID>,
        pub isPositive: Option<bool>
    }

    pub fn load_type_bonuses<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeBonuses), SDELoadError>>, SDELoadError> {
        load_file::<TypeBonuses, R>(archive, "typeBonus.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeDogma {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        #[serde(deserialize_with="deserialize_type_attributes")]
        pub dogmaAttributes: IndexMap<ids::AttributeID, f64>,
        #[serde(default)]
        #[serde(deserialize_with="deserialize_type_effects")]
        pub dogmaEffects: IndexMap<ids::EffectID, bool>
    }

    fn deserialize_type_attributes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<ids::AttributeID, f64>, D::Error> {
        struct SeqVisitor;
        impl<'de> Visitor<'de> for SeqVisitor {
            type Value = IndexMap<ids::AttributeID, f64>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of type attributes")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                struct TypeDogmaAttribute {
                    pub attributeID: ids::AttributeID,
                    pub value: f64
                }

                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<TypeDogmaAttribute>()? {
                    map.insert(value.attributeID, value.value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(SeqVisitor)
    }

    fn deserialize_type_effects<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<ids::EffectID, bool>, D::Error> {
        struct SeqVisitor;
        impl<'de> Visitor<'de> for SeqVisitor {
            type Value = IndexMap<ids::EffectID, bool>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("array of type effects")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                #[derive(Debug, Deserialize)]
                #[allow(non_snake_case)]
                #[serde(deny_unknown_fields)]
                struct TypeDogmaEffect {
                    pub effectID: ids::EffectID,
                    pub isDefault: bool
                }

                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<TypeDogmaEffect>()? {
                    map.insert(value.effectID, value.isDefault);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(SeqVisitor)
    }

    pub fn load_type_dogma<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeDogma), SDELoadError>>, SDELoadError> {
        load_file::<TypeDogma, R>(archive, "typeDogma.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeMaterials {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        #[serde(default)]
        pub materials: Vec<TypeMaterial>,
        #[serde(default)]
        pub randomizedMaterials: Vec<TypeRandomMaterial>    // TODO: Replace this with a typeID indexed map
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeMaterial {
        pub materialTypeID: ids::TypeID,
        pub quantity: u32
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct TypeRandomMaterial {
        pub materialTypeID: ids::TypeID,
        pub quantityMax: u32,
        pub quantityMin: u32,
    }

    pub fn load_type_materials<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeMaterials), SDELoadError>>, SDELoadError> {
        load_file::<TypeMaterials, R>(archive, "typeMaterials.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(deny_unknown_fields)]
    pub struct Type {
        #[serde(rename="_key")]
        pub typeID: ids::TypeID,
        pub basePrice: Option<f64>,
        pub capacity: Option<f64>,
        pub description: Option<LocalizedString>,
        pub factionID: Option<ids::FactionID>,
        pub graphicID: Option<ids::GraphicID>,
        pub groupID: ids::GroupID,
        pub iconID: Option<ids::IconID>,
        pub marketGroupID: Option<ids::MarketGroupID>,
        pub mass: Option<f64>,
        pub metaGroupID: Option<ids::MetaGroupID>,
        pub name: LocalizedString,
        pub portionSize: i32,
        pub published: bool,
        pub raceID: Option<ids::RaceID>,
        pub radius: Option<f64>,
        pub soundID: Option<ids::SoundID>,
        pub variationParentTypeID: Option<ids::TypeID>,
        pub volume: Option<f64>,
    }

    pub fn load_types<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, Type), SDELoadError>>, SDELoadError> {
        load_file::<Type, R>(archive, "types.jsonl")
            .map(|iter| iter.map(|res| res.map(|entry| (entry.typeID, entry))))
    }

    #[derive(Debug)]
    pub struct SDE {
        pub agents_in_space: IndexMap<ids::CharacterID, AgentInSpace>,
        pub agent_types: IndexMap<ids::AgentTypeID, AgentType>,
        pub ancestries: IndexMap<ids::AncestryID, Ancestry>,
        pub bloodlines: IndexMap<ids::BloodlineID, Bloodline>,
        pub blueprints: IndexMap<ids::TypeID, Blueprint>,
        pub categories: IndexMap<ids::CategoryID, Category>,
        pub certificates: IndexMap<ids::CertificateID, Certificate>,
        pub character_attributes: IndexMap<ids::CharacterAttributeID, CharacterAttribute>,
        pub contraband_types: IndexMap<ids::TypeID, ContrabandType>,
        pub control_tower_resources: IndexMap<ids::TypeID, ControlTowerResources>,
        pub corporation_activities: IndexMap<ids::CorporationActivityID, CorporationActivity>,
        pub dbuff_collections: IndexMap<ids::WarfareBuffID, WarfareBuff>,
        pub dogma_attribute_categories: IndexMap<ids::AttributeCategoryID, AttributeCategory>,
        pub dogma_attributes: IndexMap<ids::AttributeID, Attribute>,
        pub dogma_effects: IndexMap<ids::EffectID, Effect>,
        pub dogma_units: IndexMap<ids::UnitID, DogmaUnit>,
        pub dynamic_item_attributes: IndexMap<ids::TypeID, DynamicItemAttributes>,
        pub factions: IndexMap<ids::FactionID, Faction>,
        pub graphics: IndexMap<ids::GraphicID, Graphic>,
        pub groups: IndexMap<ids::GroupID, Group>,
        pub icons: IndexMap<ids::IconID, Icon>,
        pub landmarks: IndexMap<ids::LandmarkID, Landmark>,
        pub map_asteroid_belts: IndexMap<ids::AsteroidBeltID, AsteroidBelt>,
        pub map_constellations: IndexMap<ids::ConstellationID, Constellation>,
        pub map_moons: IndexMap<ids::MoonID, Moon>,
        pub map_planets: IndexMap<ids::PlanetID, Planet>,
        pub map_regions: IndexMap<ids::RegionID, Region>,
        pub map_solarsystems: IndexMap<ids::SolarSystemID, SolarSystem>,
        pub map_stargates: IndexMap<ids::StargateID, Stargate>,
        pub map_stars: IndexMap<ids::StarID, Star>,
        pub market_groups: IndexMap<ids::MarketGroupID, MarketGroup>,
        pub masteries: IndexMap<ids::TypeID, MasteryLevels>,
        pub meta_groups: IndexMap<ids::MetaGroupID, MetaGroup>,
        pub npc_characters: IndexMap<ids::CharacterID, NpcCharacter>,
        pub npc_corporation_divisions: IndexMap<ids::DivisionID, NpcCorporationDivision>,
        pub npc_corporations: IndexMap<ids::CorporationID, NpcCorporation>,
        pub npc_stations: IndexMap<ids::StationID, NpcStation>,
        pub planet_resources: IndexMap<ids::PlanetID, PlanetResource>,
        pub planet_schematics: IndexMap<ids::PlanetSchematicID, PlanetSchematic>,
        pub races: IndexMap<ids::RaceID, CharacterRace>,
        pub skin_licenses: IndexMap<ids::TypeID, SkinLicense>,
        pub skin_materials: IndexMap<ids::SkinMaterialID, SkinMaterial>,
        pub skins: IndexMap<ids::SkinID, Skin>,
        pub sovereignty_upgrades: IndexMap<ids::TypeID, SovereigntyUpgrade>,
        pub station_operations: IndexMap<ids::StationOperationID, StationOperation>,
        pub station_services: IndexMap<ids::StationServiceID, StationService>,
        pub translation_languages: Vec<TranslationLanguage>,
        pub type_bonus: IndexMap<ids::TypeID, TypeBonuses>,
        pub type_dogma: IndexMap<ids::TypeID, TypeDogma>,
        pub type_materials: IndexMap<ids::TypeID, TypeMaterials>,
        pub types: IndexMap<ids::TypeID, Type>,
    }

    pub fn load_all<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<SDE, SDELoadError> {
        Ok(SDE {
            // Curly braces are mandatory here to make rustc see that the iterator from `load_file`, which retains a borrow on `archive`, is consumed/dropped by the `collect` call
            agents_in_space: { load_agents_in_space(archive)?.collect::<Result<_, _>>()? },
            agent_types: { load_agent_types(archive)?.collect::<Result<_, _>>()? },
            ancestries: { load_ancestries(archive)?.collect::<Result<_, _>>()? },
            bloodlines: { load_bloodlines(archive)?.collect::<Result<_, _>>()? },
            blueprints: { load_blueprints(archive)?.collect::<Result<_, _>>()? },
            categories: { load_categories(archive)?.collect::<Result<_, _>>()? },
            certificates: { load_certificates(archive)?.collect::<Result<_, _>>()? },
            character_attributes: { load_character_attributes(archive)?.collect::<Result<_, _>>()? },
            contraband_types: { load_contraband_types(archive)?.collect::<Result<_, _>>()? },
            control_tower_resources: { load_controltower_resources(archive)?.collect::<Result<_, _>>()?},
            corporation_activities: { load_corporation_activities(archive)?.collect::<Result<_, _>>()? },
            dbuff_collections: { load_dbuff_collections(archive)?.collect::<Result<_, _>>()? },
            dogma_attribute_categories: { load_dogma_attribute_categories(archive)?.collect::<Result<_, _>>()? },
            dogma_attributes: { load_dogma_attributes(archive)?.collect::<Result<_, _>>()? },
            dogma_effects: { load_dogma_effects(archive)?.collect::<Result<_, _>>()? },
            dogma_units: { load_dogma_units(archive)?.collect::<Result<_, _>>()? },
            dynamic_item_attributes: { load_dynamic_item_attributes(archive)?.collect::<Result<_, _>>()? },
            factions: { load_factions(archive)?.collect::<Result<_, _>>()? },
            graphics: { load_graphics(archive)?.collect::<Result<_, _>>()? },
            groups: { load_groups(archive)?.collect::<Result<_, _>>()? },
            icons: { load_icons(archive)?.collect::<Result<_, _>>()? },
            landmarks: { load_landmarks(archive)?.collect::<Result<_, _>>()? },
            map_asteroid_belts: { load_asteroid_belts(archive)?.collect::<Result<_, _>>()? },
            map_constellations: { load_constellations(archive)?.collect::<Result<_, _>>()? },
            map_moons: { load_moons(archive)?.collect::<Result<_, _>>()? },
            map_planets: { load_planets(archive)?.collect::<Result<_, _>>()? },
            map_regions: { load_regions(archive)?.collect::<Result<_, _>>()? },
            map_solarsystems: { load_solarsystems(archive)?.collect::<Result<_, _>>()? },
            map_stargates: { load_stargates(archive)?.collect::<Result<_, _>>()? },
            map_stars: { load_stars(archive)?.collect::<Result<_, _>>()? },
            market_groups: { load_market_groups(archive)?.collect::<Result<_, _>>()? },
            masteries: { load_masteries(archive)?.collect::<Result<_, _>>()? },
            meta_groups: { load_meta_groups(archive)?.collect::<Result<_, _>>()? },
            npc_characters: { load_npc_characters(archive)?.collect::<Result<_, _>>()? },
            npc_corporation_divisions: { load_npc_corporation_divisions(archive)?.collect::<Result<_, _>>()? },
            npc_corporations: { load_npc_corporations(archive)?.collect::<Result<_, _>>()? },
            npc_stations: { load_npc_stations(archive)?.collect::<Result<_, _>>()? },
            planet_resources: { load_planet_resources(archive)?.collect::<Result<_, _>>()? },
            planet_schematics: { load_planet_schematics(archive)?.collect::<Result<_, _>>()? },
            races: { load_races(archive)?.collect::<Result<_, _>>()? },
            skin_licenses: { load_skin_licenses(archive)?.collect::<Result<_, _>>()? },
            skin_materials: { load_skin_materials(archive)?.collect::<Result<_, _>>()? },
            skins: { load_skins(archive)?.collect::<Result<_, _>>()? },
            sovereignty_upgrades: { load_sovereignty_upgrades(archive)?.collect::<Result<_, _>>()? },
            station_operations: { load_station_operations(archive)?.collect::<Result<_, _>>()? },
            station_services: { load_station_services(archive)?.collect::<Result<_, _>>()? },
            translation_languages: { load_translation_languages(archive)?.collect::<Result<_, _>>()? },
            type_bonus: { load_type_bonuses(archive)?.collect::<Result<_, _>>()? },
            type_dogma: { load_type_dogma(archive)?.collect::<Result<_, _>>()? },
            type_materials: { load_type_materials(archive)?.collect::<Result<_, _>>()? },
            types: { load_types(archive)?.collect::<Result<_, _>>()? },
        })
    }
}

#[cfg(feature="update")]
#[allow(non_snake_case, non_camel_case_types)] // Use of serialized types, whose names match the output fields
pub mod update {
    use serde::{Deserialize, Serialize};
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use std::{fs, io};
    use zip::ZipArchive;

    pub const VERSION_URL: &'static str = "https://developers.eveonline.com/static-data/tranquility/latest.jsonl";
    pub const SDE_URL: &'static str = "https://developers.eveonline.com/static-data/eve-online-static-data-latest-jsonl.zip";

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "_key")]
    pub enum SdeVersion {
        sde { buildNumber: u32, releaseDate: String }
    }

    impl SdeVersion {
        pub fn try_zip<P: AsRef<Path>>(path: P) -> Result<SdeVersion, io::Error> {
            if fs::exists(&path)? {
                #[allow(unused_qualifications)]
                Self::from_sde(path)
            } else {
                Ok(SdeVersion::sde { buildNumber: 0, releaseDate: "".to_string() })
            }
        }

        pub fn from_sde<P: AsRef<Path>>(path: P) -> Result<SdeVersion, io::Error> {
            let mut archive = ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
            serde_json::from_reader(archive.by_name("_sde.jsonl").map_err(io::Error::other)?).map_err(io::Error::other)
        }

        pub fn from_file<R: Read>(read: R) -> Result<SdeVersion, io::Error> {
            serde_json::from_reader(read).map_err(io::Error::other)
        }

        pub fn download_latest() -> Result<SdeVersion, io::Error> {
            reqwest::blocking::get(VERSION_URL).map_err(io::Error::other)?
                .json::<SdeVersion>().map_err(io::Error::other)
        }
    }

    pub fn download_latest_sde<P: AsRef<Path>>(file: P) -> Result<SdeVersion, io::Error> {
        reqwest::blocking::get(SDE_URL).map_err(io::Error::other)?
            .copy_to(&mut File::create(&file)?).map(|_| ()).map_err(io::Error::other)?;

        SdeVersion::try_zip(file)
    }

    pub fn update_sde<P: AsRef<Path>>(file: P) -> Result<SdeVersion, io::Error> {
        let current @ SdeVersion::sde { buildNumber: current_version, .. } = SdeVersion::try_zip(&file)?;
        let SdeVersion::sde { buildNumber: latest, .. } = SdeVersion::download_latest()?;
        if current_version < latest {
            download_latest_sde(file)
        } else {
            Ok(current)
        }
    }
}
