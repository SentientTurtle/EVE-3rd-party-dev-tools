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

    #[derive(Debug)]
    pub enum SDELoadError {
        /// IO Error while reading from SDE
        IO(io::Error),
        /// An error occurred parsing the .zip file; Archive corrupt?
        Zip(ZipError),
        /// SDE zip file did not contain expected file, did the SDE format change?
        ArchiveFileNotFound(String),
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
        Ok(std::iter::from_fn(move || {
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

    fn deserialize_inline_entry_map<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de>, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<K, V>, D::Error> {
        struct EntryVisitor<K, V>(PhantomData<K>, PhantomData<V>);
        impl<'de, K: Deserialize<'de> + Hash + Eq + Ord, V: Deserialize<'de>> Visitor<'de> for EntryVisitor<K, V> {
            type Value = IndexMap<K, V>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a map encoded as array of flattened entries")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
                let size_hint = seq.size_hint();
                let mut map = size_hint.map(IndexMap::with_capacity).unwrap_or_else(IndexMap::new);
                while let Some(value) = seq.next_element::<InlineEntry<K, V>>()? {
                    map.insert(value._key, value.value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(EntryVisitor::<K, V>(PhantomData::default(), PhantomData::default()))
    }

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
                while let Some(value) = seq.next_element::<ExplicitEntry<K, V>>()? {
                    map.insert(value._key, value._value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(EntryVisitor::<K, V>(PhantomData::default(), PhantomData::default()))
    }

    // Generic types
    #[derive(Deserialize)]
    pub struct InlineEntry<K, V> {
        _key: K,
        #[serde(flatten)]
        value: V
    }

    impl<K, V> InlineEntry<K, V> {
        #[inline(always)]
        pub fn tuple(self) -> (K, V) {
            (self._key, self.value)
        }
    }

    impl<K: Hash + Eq + Ord, V> FromIterator<InlineEntry<K, V>> for IndexMap<K, V> {
        fn from_iter<I: IntoIterator<Item=InlineEntry<K, V>>>(iter: I) -> Self {
            IndexMap::<K, V>::from_iter(iter.into_iter().map(InlineEntry::tuple))
        }
    }

    #[derive(Deserialize)]
    pub struct ExplicitEntry<K, V> {
        _key: K,
        _value: V
    }

    impl<K, V> ExplicitEntry<K, V> {
        #[inline(always)]
        pub fn tuple(self) -> (K, V) {
            (self._key, self._value)
        }
    }

    impl<K: Hash + Eq + Ord, V> FromIterator<ExplicitEntry<K, V>> for IndexMap<K, V> {
        fn from_iter<I: IntoIterator<Item=ExplicitEntry<K, V>>>(iter: I) -> Self {
            IndexMap::<K, V>::from_iter(iter.into_iter().map(ExplicitEntry::tuple))
        }
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Position {
        pub x: f64,
        pub y: f64,
        pub z: f64
    }

    #[derive(Debug, Deserialize)]
    pub struct LocalizedString {
        pub en: String,
        pub de: Option<String>,
        pub es: Option<String>,
        pub fr: Option<String>,
        pub ja: Option<String>,
        pub ko: Option<String>,
        pub ru: Option<String>,
        pub zh: Option<String>,
        pub it: Option<String>,
    }

    // SDE Entry types

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct AgentInSpace {
        pub dungeonID: ids::DungeonID,
        pub solarSystemID: ids::SolarSystemID,
        pub spawnPointID: ids::SpawnPointID,
        pub typeID: ids::TypeID
    }
    pub fn load_agents_in_space<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterID, AgentInSpace), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "agentsInSpace.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    pub enum AgentType {
        NonAgent, BasicAgent, TutorialAgent, ResearchAgent, CONCORDAgent, GenericStorylineMissionAgent, StorylineMissionAgent, EventMissionAgent, FactionalWarfareAgent, EpicArcAgent, AuraAgent, CareerAgent, HeraldryAgent
    }

    #[derive(Debug, Deserialize)]
    struct AgentTypeWrapper {
        pub name: AgentType
    }

    pub fn load_agent_types<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AgentTypeID, AgentType), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, AgentTypeWrapper>, R>(archive, "agentTypes.jsonl")
            .map(|iter| iter.map(|value| value.map(|entry| (entry._key, entry.value.name))))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Ancestry {
        pub bloodlineID: ids::BloodlineID,
        pub charisma: i32,
        pub intelligence: i32,
        pub memory: i32,
        pub perception: i32,
        pub willpower: i32,
        pub description: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub name: LocalizedString,
        pub shortDescription: Option<String>
    }

    pub fn load_ancestries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AncestryID, Ancestry), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "ancestries.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Bloodline {
        pub corporationID: ids::CorporationID,
        pub description: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub name: LocalizedString,
        pub raceID: ids::RaceID,
        pub charisma: i32,
        pub intelligence: i32,
        pub memory: i32,
        pub perception: i32,
        pub willpower: i32,
    }

    pub fn load_bloodlines<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::BloodlineID, Bloodline), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "bloodlines.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Blueprint {
        pub blueprintTypeID: ids::TypeID,
        pub maxProductionLimit: i32,
        pub activities: BlueprintActivities
    }
    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
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
    pub struct BPActivity {
        #[serde(deserialize_with="deserialize_activity_materials", default)]
        pub materials: IndexMap<ids::TypeID, u32>,
        #[serde(deserialize_with="deserialize_activity_products", default)]
        pub products: IndexMap<ids::TypeID, (u32, Option<f64>)>,
        #[serde(deserialize_with="deserialize_activity_skills", default)]
        pub skills: IndexMap<ids::TypeID, numbers::SkillLevel>,
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
        load_file::<InlineEntry<_, _>, R>(archive, "blueprints.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }


    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Category {
        pub name: LocalizedString,
        pub published: bool,
        pub iconID: Option<ids::IconID>
    }

    pub fn load_categories<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CategoryID, Category), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "categories.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Certificate {
        pub groupID: ids::GroupID,  // TODO: Double-check that this refers to item-groups
        pub name: LocalizedString,
        pub description: LocalizedString,
        #[serde(default)]
        pub recommendedFor: Vec<ids::TypeID>,
        #[serde(deserialize_with="deserialize_inline_entry_map")]
        pub skillTypes: IndexMap<ids::TypeID, CertificateSkillLevels>
    }

    #[derive(Debug, Deserialize)]
    pub struct CertificateSkillLevels {
        pub basic: numbers::SkillLevel,
        pub standard: numbers::SkillLevel,
        pub improved: numbers::SkillLevel,
        pub advanced: numbers::SkillLevel,
        pub elite: numbers::SkillLevel,
    }

    pub fn load_certificates<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CertificateID, Certificate), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "certificates.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct CharacterAttribute {
        pub name: LocalizedString,
        pub description: String,
        pub iconID: ids::IconID,
        pub notes: String,
        pub shortDescription: String
    }

    pub fn load_character_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterAttributeID, CharacterAttribute), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "characterAttributes.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct ContrabandType {
        #[serde(deserialize_with="deserialize_inline_entry_map")]
        pub factions: IndexMap<ids::FactionID, ContrabandTypeFaction>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct ContrabandTypeFaction {
        pub attackMinSec: f64,
        pub confiscateMinSec: f64,
        pub fineByValue: f64,
        pub standingLoss: f64
    }

    pub fn load_contraband_types<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, ContrabandType), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "contrabandTypes.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct ControlTowerResource {
        pub resources: Vec<ControlTowerResourceResource>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct ControlTowerResourceResource {
        pub purpose: u8,
        pub quantity: u32,
        pub resourceTypeID: ids::TypeID,
        pub factionID: Option<ids::FactionID>,  // Fuel required if in faction's space
        pub minSecurityLevel: Option<f64>   // Can't use default here as security can be less than zero.
    }

    pub fn load_controltower_resources<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, ControlTowerResource), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "controlTowerResources.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct CorporationActivity {
        pub name: LocalizedString
    }

    pub fn load_corporation_activities<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CorporationActivityID, CorporationActivity), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "corporationActivities.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct WarfareBuff {
        pub aggregateMode: WarfareBuffAggregateMode,
        pub developerDescription: String,
        pub displayName: Option<LocalizedString>,
        pub itemModifiers: Option<Vec<WarfareBuffItemModifier>>,
        pub locationGroupModifiers: Option<Vec<WarfareBuffLocationGroupModifier>>,
        pub locationModifiers: Option<Vec<WarfareBuffLocationModifier>>,
        pub locationRequiredSkillModifiers: Option<Vec<WarfareBuffLocationRequiredSkillModifier>>,
        pub operationName: WarfareBuffOperation,
        pub showOutputValueInUI: WarfareBuffUIMode
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub enum WarfareBuffAggregateMode {
        Maximum, Minimum
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub enum WarfareBuffOperation {
        PostMul, PostPercent, ModAdd, PostAssignment
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub enum WarfareBuffUIMode {
        ShowNormal, Hide, ShowInverted
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct WarfareBuffItemModifier {
        pub dogmaAttributeID: ids::AttributeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct WarfareBuffLocationGroupModifier {
        pub dogmaAttributeID: ids::AttributeID,
        pub groupID: ids::GroupID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct WarfareBuffLocationModifier {
        pub dogmaAttributeID: ids::AttributeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct WarfareBuffLocationRequiredSkillModifier {
        pub dogmaAttributeID: ids::AttributeID,
        pub skillID: ids::TypeID
    }

    pub fn load_dbuff_collections<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::WarfareBuffID, WarfareBuff), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "dbuffCollections.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct AttributeCategory {
        pub name: String,
        pub description: Option<String>
    }

    pub fn load_dogma_attribute_categories<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AttributeCategoryID, AttributeCategory), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "dogmaAttributeCategories.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Attribute {
        pub attributeCategoryID: Option<ids::AttributeCategoryID>,
        pub chargeRechargeTimeID: Option<u32>,    // TODO: Unknown ID
        pub dataType: i32,  // TODO: What's this?
        pub defaultValue: f64,
        pub description: Option<String>,
        pub displayName: Option<LocalizedString>,
        pub displayWhenZero: bool,
        pub highIsGood: bool,
        pub iconID: Option<ids::IconID>,
        pub maxAttributeID: Option<ids::AttributeID>,
        pub minAttributeID: Option<ids::AttributeID>,
        pub name: String,
        pub published: bool,
        pub stackable: bool,
        pub tooltipDescription: Option<LocalizedString>,
        pub tooltipTitle: Option<LocalizedString>,
        pub unitID: Option<ids::UnitID>,
    }

    pub fn load_dogma_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::AttributeID, Attribute), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "dogmaAttributes.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Effect {
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
        pub modifierInfo: Option<Vec<ModifierInfo>>,
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
        load_file::<InlineEntry<_, _>, R>(archive, "dogmaEffects.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct DogmaUnit {
        pub description: Option<LocalizedString>,
        pub displayName: Option<LocalizedString>,
        pub name: String,
    }

    pub fn load_dogma_units<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::UnitID, DogmaUnit), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "dogmaUnits.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct DynamicItemAttributes {
        #[serde(deserialize_with="deserialize_inline_entry_map")]
        pub attributeIDs: IndexMap<ids::AttributeID, DynamicItemAttributesAttribute>,
        pub inputOutputMapping: Vec<DynamicItemAttributesIOMapping>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct DynamicItemAttributesAttribute {
        pub highIsGood: Option<bool>,
        pub max: f64,
        pub min: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct DynamicItemAttributesIOMapping {
        pub applicableTypes: Vec<ids::TypeID>,
        pub resultingType: ids::TypeID
    }

    pub fn load_dynamic_item_attributes<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, DynamicItemAttributes), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "dynamicItemAttributes.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Faction {
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
        load_file::<InlineEntry<_, _>, R>(archive, "factions.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Graphic {
        pub graphicFile: Option<String>,
        pub iconFolder: Option<String>,
        pub sofFactionName: Option<String>,
        pub sofHullName: Option<String>,
        pub sofLayout: Option<Vec<String>>,
        pub sofMaterialSetID: Option<ids::MaterialSetID>,
        pub sofRaceName: Option<String>,
    }

    pub fn load_graphics<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::GraphicID, Graphic), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "graphics.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Group {
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
        load_file::<InlineEntry<_, _>, R>(archive, "groups.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Icon {
        pub iconFile: String
    }

    pub fn load_icons<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::IconID, Icon), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "icons.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Landmark {
        pub description: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub locationID: Option<ids::LocationID>,
        pub name: LocalizedString,
        pub position: Position
    }

    pub fn load_landmarks<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::LandmarkID, Landmark), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "landmarks.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct AsteroidBelt {
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
        load_file::<InlineEntry<_, _>, R>(archive, "mapAsteroidBelts.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Constellation {
        pub regionID: ids::RegionID,
        pub factionID: Option<ids::FactionID>,
        pub position: Position,
        pub name: LocalizedString,
        pub solarSystemIDs: Vec<ids::SolarSystemID>,
        pub wormholeClassID: Option<ids::WormholeClassID>
    }

    pub fn load_constellations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::ConstellationID, Constellation), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapConstellations.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Moon {
        pub attributes: MoonAttributes,
        pub celestialIndex: u32,
        pub npcStationIDs: Option<Vec<ids::StationID>>,
        pub orbitID: ids::ItemID,
        pub orbitIndex: u32,
        pub position: Position,
        pub radius: f64,
        pub statistics: Option<MoonStatistics>,
        pub typeID: ids::TypeID,
        pub uniqueName: Option<LocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
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
    pub struct MoonAttributes {
        pub heightMap1: u32,
        pub heightMap2: u32,
        pub shaderPreset: u32
    }

    pub fn load_moons<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MoonID, Moon), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapMoons.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Planet {
        pub asteroidBeltIDs: Option<Vec<ids::AsteroidBeltID>>,
        pub attributes: PlanetAttributes,
        pub celestialIndex: u32,
        pub moonIDs: Option<Vec<ids::MoonID>>,
        pub npcStationIDs: Option<Vec<ids::StationID>>,
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
    pub struct PlanetAttributes {
        pub heightMap1: u32,
        pub heightMap2: u32,
        pub population: bool,
        pub shaderPreset: u32
    }

    pub fn load_planets<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetID, Planet), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapPlanets.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Region {
        pub constellationIDs: Vec<ids::ConstellationID>,
        pub description: Option<LocalizedString>,
        pub factionID: Option<ids::FactionID>,
        pub name: LocalizedString,
        pub nebulaID: u32,    // TODO: Assign ID type
        pub position: Position,
        pub wormholeClassID: Option<ids::WormholeClassID>
    }

    pub fn load_regions<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::RegionID, Region), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapRegions.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct SolarSystem {
        pub border: Option<bool>,
        pub constellationID: ids::ConstellationID,
        pub corridor: Option<bool>,
        pub disallowedAnchorCategories: Option<Vec<ids::CategoryID>>,
        pub disallowedAnchorGroups: Option<Vec<ids::GroupID>>,
        pub factionID: Option<ids::FactionID>,
        pub fringe: Option<bool>,
        pub hub: Option<bool>,
        pub international: Option<bool>,
        pub luminosity: Option<f64>,
        pub name: LocalizedString,
        pub planetIDs: Option<Vec<ids::PlanetID>>,
        pub position: Position,
        pub radius: f64,
        pub regionID: ids::RegionID,
        pub regional: Option<bool>,
        // pub secondarySun: Option<SecondarySun>, Removed T.T CCPls
        pub securityClass: Option<String>,
        pub securityStatus: f64,
        pub starID: Option<ids::StarID>,
        pub stargateIDs: Option<Vec<ids::StargateID>>,
        pub visualEffect: Option<String>,
        pub wormholeClassID: Option<ids::WormholeClassID>,
    }

    pub fn load_solarsystems<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::SolarSystemID, SolarSystem), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapSolarSystems.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Stargate {
        pub destination: StargateDestination,
        pub position: Position,
        pub solarSystemID: ids::SolarSystemID,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct StargateDestination {
        pub solarSystemID: ids::SolarSystemID,
        pub stargateID: ids::StargateID
    }

    pub fn load_stargates<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StargateID, Stargate), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapStargates.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Star {
        pub radius: f64,
        pub solarSystemID: ids::SolarSystemID,
        pub statistics: StarStatistics,
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct StarStatistics {
        pub age: f64,
        pub life: f64,
        pub luminosity: f64,
        pub spectralClass: String,
        pub temperature: f64
    }

    pub fn load_stars<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StarID, Star), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "mapStars.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct MarketGroup {
        pub description: Option<LocalizedString>,
        pub hasTypes: bool,
        pub iconID: Option<ids::IconID>,
        pub name: LocalizedString,
        pub parentGroupID: Option<ids::MarketGroupID>
    }

    pub fn load_market_groups<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MarketGroupID, MarketGroup), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "marketGroups.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
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

                    while let Some(ExplicitEntry { _key, _value }) = seq.next_element::<ExplicitEntry<u8, Vec<ids::CertificateID>>>()? {
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
        load_file::<ExplicitEntry<_, _>, R>(archive, "masteries.jsonl")
            .map(|iter| iter.map(|value| value.map(ExplicitEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct MetaGroup {
        pub color: Option<MetaGroupColor>,
        pub name: LocalizedString,
        pub iconID: Option<ids::IconID>,
        pub iconSuffix: Option<String>,
        pub description: Option<LocalizedString>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct MetaGroupColor {
        pub r: f64,
        pub g: f64,
        pub b: f64,
    }

    pub fn load_meta_groups<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::MetaGroupID, MetaGroup), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "metaGroups.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcCharacter {
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
        pub skills: Option<Vec<NpcCharacterSkill>>,
        pub specialtyID: Option<ids::SpecialtyID>,
        pub startDate: Option<String>,
        pub uniqueName: bool
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcCharacterSkill {
        pub typeID: ids::TypeID
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcCharacterAgent {
        pub agentTypeID: ids::TypeID,
        pub divisionID: ids::DivisionID,
        pub isLocator: bool,
        pub level: i32,
    }

    pub fn load_npc_characters<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CharacterID, NpcCharacter), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "npcCharacters.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcCorporationDivision {
        pub description: Option<LocalizedString>,
        pub displayName: Option<String>,
        pub internalName: String,
        pub leaderTypeName: LocalizedString,
        pub name: LocalizedString
    }

    pub fn load_npc_corporation_divisions<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::DivisionID, NpcCorporationDivision), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "npcCorporationDivisions.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcCorporation {
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
        pub lpOfferTables: Option<Vec<u32>>,    // TODO: Assign ID type
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
    pub struct CorporationDivision {
        pub divisionNumber: i32,
        pub leaderID: ids::CharacterID,
        pub size: i32
    }

    pub fn load_npc_corporations<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::CorporationID, NpcCorporation), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "npcCorporations.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct NpcStation {
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
        load_file::<InlineEntry<_, _>, R>(archive, "npcStations.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    #[serde(untagged)]
    pub enum PlanetResource {
        Star { power: i32, },
        ResourcePlanet { workforce: i32 },
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

    pub fn load_planet_resources<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetID, PlanetResource), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "planetResources.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct PlanetSchematic {
        pub cycleTime: u32,
        pub name: LocalizedString,
        pub pins: Vec<ids::TypeID>,
        #[serde(default, deserialize_with="deserialize_inline_entry_map")]
        pub types: IndexMap<ids::TypeID, PlanetSchematicType>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct PlanetSchematicType {
        pub isInput: bool,
        pub quantity: u32
    }

    pub fn load_planet_schematics<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::PlanetSchematicID, PlanetSchematic), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "planetSchematics.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct CharacterRace {
        pub name: LocalizedString,
        pub description: Option<LocalizedString>,
        pub iconID: Option<ids::IconID>,
        pub shipTypeID: Option<ids::TypeID>, // Corvette/"Rookie ship"
        #[serde(default, deserialize_with="deserialize_explicit_entry_map")]
        pub skills: IndexMap<ids::TypeID, numbers::SkillLevel>
    }

    pub fn load_races<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::RaceID, CharacterRace), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "races.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct SkinLicense {
        pub duration: i32,
        pub licenseTypeID: ids::TypeID,
        pub skinID: ids::SkinID,
        pub isSingleUse: Option<bool>
    }

    pub fn load_skin_licenses<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, SkinLicense), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "skinLicenses.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct SkinMaterial {
        pub displayName: Option<LocalizedString>,
        pub materialSetID: ids::MaterialSetID,
    }

    pub fn load_skin_materials<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::SkinMaterialID, SkinMaterial), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "skinMaterials.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Skin {
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
        load_file::<InlineEntry<_, _>, R>(archive, "skins.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct SovereigntyUpgrade {
        pub mutually_exclusive_group: String,
        pub power_allocation: i32,
        pub workforce_allocation: i32,
        pub fuel_type_id: Option<ids::TypeID>,
        pub fuel_startup_cost: Option<i32>,
        pub fuel_hourly_upkeep: Option<i32>
    }

    pub fn load_sovereignty_upgrades<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, SovereigntyUpgrade), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "sovereigntyUpgrades.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct StationOperation {
        pub activityID: ids::StationActivityID,
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
        load_file::<InlineEntry<_, _>, R>(archive, "stationOperations.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct StationService {
        pub serviceName: LocalizedString,
        pub description: Option<LocalizedString>,
    }

    pub fn load_station_services<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::StationServiceID, StationService), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "stationServices.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd)]
    #[allow(non_snake_case, non_camel_case_types)]
    pub enum TranslationLanguage {
        en,
        de,
        es,
        fr,
        ja,
        ko,
        ru,
        zh,
        it
    }

    #[derive(Debug, Deserialize, Hash, Eq, PartialEq)]
    #[allow(non_snake_case)]
    pub struct TranslationLanguageName {
        pub name: String
    }

    pub fn load_translation_languages<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(TranslationLanguage, TranslationLanguageName), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "translationLanguages.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeBonuses {   // Kinds of bonuses may be omitted, an empty collection is given for those
        pub iconID: Option<ids::IconID>,
        pub miscBonuses: Option<Vec<TypeBonus>>,
        pub roleBonuses: Option<Vec<TypeBonus>>,
        #[serde(default, rename = "types", deserialize_with="deserialize_explicit_entry_map")]
        pub skillBonuses: IndexMap<ids::TypeID, Vec<TypeBonus>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeBonus {
        pub bonusText: LocalizedString,
        pub importance: i32,
        pub bonus: Option<f64>,
        pub unitID: Option<ids::UnitID>,
        pub isPositive: Option<bool>
    }

    pub fn load_type_bonuses<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeBonuses), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "typeBonus.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeDogma {
        pub dogmaAttributes: Vec<TypeDogmaAttribute>,   // TODO: Convert to map
        #[serde(default)]
        pub dogmaEffects: Vec<TypeDogmaEffect>          // TODO: Convert to map
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeDogmaAttribute {
        pub attributeID: ids::AttributeID,
        pub value: f64
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeDogmaEffect {
        pub effectID: ids::EffectID,
        pub isDefault: bool
    }

    pub fn load_type_dogma<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeDogma), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "typeDogma.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeMaterials {
        pub materials: Vec<TypeMaterial>
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct TypeMaterial {
        pub materialTypeID: ids::TypeID,
        pub quantity: u32
    }

    pub fn load_type_materials<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<impl Iterator<Item=Result<(ids::TypeID, TypeMaterials), SDELoadError>>, SDELoadError> {
        load_file::<InlineEntry<_, _>, R>(archive, "typeMaterials.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
    }

    #[derive(Debug, Deserialize)]
    #[allow(non_snake_case)]
    pub struct Type {
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
        load_file::<InlineEntry<_, _>, R>(archive, "types.jsonl")
            .map(|iter| iter.map(|value| value.map(InlineEntry::tuple)))
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
        pub control_tower_resources: IndexMap<ids::TypeID, ControlTowerResource>,
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
        pub translation_languages: IndexMap<TranslationLanguage, TranslationLanguageName>,
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

    #[derive(Serialize, Deserialize)]
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

    pub fn download_latest_sde<P: AsRef<Path>>(file: P) -> Result<(), io::Error> {
        reqwest::blocking::get(SDE_URL).map_err(io::Error::other)?
            .copy_to(&mut File::create(file)?)
            .map(|_| ()).map_err(io::Error::other)
    }

    pub fn update_sde<P: AsRef<Path>>(file: P) -> Result<(), io::Error> {
        let SdeVersion::sde { buildNumber: current_version, .. } = SdeVersion::try_zip(&file)?;
        let SdeVersion::sde { buildNumber: latest, .. } = SdeVersion::download_latest()?;
        if current_version < latest {
            download_latest_sde(file)
        } else {
            Ok(())
        }
    }
}
