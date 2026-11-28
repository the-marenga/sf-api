use enum_map::{Enum, EnumMap};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumIter;

/// All the deeds of glory a player can unlock. Each deed has to be unlocked on
/// three trigger levels (bronze, silver & gold), which is why every deed
/// appears three times in this list. The discriminants are the ids the server
/// uses to refer to a specific deed trigger level.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    FromPrimitive,
    Enum,
    EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum DeedType {
    None = 0,
    SoulEaterBronze = 1,
    SoulEaterSilver = 2,
    SoulEaterGold = 3,
    PvPGladiatorBronze = 4,
    PvPGladiatorSilver = 5,
    PvPGladiatorGold = 6,
    DungeonMasterBronze = 7,
    DungeonMasterSilver = 8,
    DungeonMasterGold = 9,
    DungeonJesusBronze = 10,
    DungeonJesusSilver = 11,
    DungeonJesusGold = 12,
    LuckOrGloryBronze = 13,
    LuckOrGlorySilver = 14,
    LuckOrGloryGold = 15,
    FortressArchitectBronze = 16,
    FortressArchitectSilver = 17,
    FortressArchitectGold = 18,
    FortressStonedBronze = 19,
    FortressStonedSilver = 20,
    FortressStonedGold = 21,
    RepDigitBronze = 22,
    RepDigitSilver = 23,
    RepDigitGold = 24,
    SleepNEarnBronze = 25,
    SleepNEarnSilver = 26,
    SleepNEarnGold = 27,
    HeroPointsBronze = 28,
    HeroPointsSilver = 29,
    HeroPointsGold = 30,
    FortyTwoHeroPointsBronze = 31,
    FortyTwoHeroPointsSilver = 32,
    FortyTwoHeroPointsGold = 33,
    BeersBronze = 34,
    BeersSilver = 35,
    BeersGold = 36,
    AttributesStrengthBronze = 37,
    AttributesStrengthSilver = 38,
    AttributesStrengthGold = 39,
    AttributesDexterityBronze = 40,
    AttributesDexteritySilver = 41,
    AttributesDexterityGold = 42,
    AttributeIntelligenceBronze = 43,
    AttributeIntelligenceSilver = 44,
    AttributeIntelligenceGold = 45,
    WhirlwindWillyBronze = 46,
    WhirlwindWillySilver = 47,
    WhirlwindWillyGold = 48,
    GoldDiggerBronze = 49,
    GoldDiggerSilver = 50,
    GoldDiggerGold = 51,
    ImmortalLegendBronze = 52,
    ImmortalLegendSilver = 53,
    ImmortalLegendGold = 54,
    HellevatorPentagramBronze = 55,
    HellevatorPentagramSilver = 56,
    HellevatorPentagramGold = 57,
    AchieverBronze = 58,
    AchieverSilver = 59,
    AchieverGold = 60,
    BrewmasterBronze = 61,
    BrewmasterSilver = 62,
    BrewmasterGold = 63,
    DiceGameCloverBronze = 64,
    DiceGameCloverSilver = 65,
    DiceGameCloverGold = 66,
    ProfessorGadgetBronze = 67,
    ProfessorGadgetSilver = 68,
    ProfessorGadgetGold = 69,
    GuildOneForAllBronze = 70,
    GuildOneForAllSilver = 71,
    GuildOneForAllGold = 72,
    PetTamerBronze = 73,
    PetTamerSilver = 74,
    PetTamerGold = 75,
    EpicOutfitBronze = 76,
    EpicOutfitSilver = 77,
    EpicOutfitGold = 78,
    ToiletHaloBronze = 79,
    ToiletHaloSilver = 80,
    ToiletHaloGold = 81,
    CollectionRaffToothBronze = 82,
    CollectionRaffToothSilver = 83,
    CollectionRaffToothGold = 84,
    RunesSecretSignBronze = 85,
    RunesSecretSignSilver = 86,
    RunesSecretSignGold = 87,
    UnderworldDeepDigBronze = 88,
    UnderworldDeepDigSilver = 89,
    UnderworldDeepDigGold = 90,
    FortressMisfortuneBronze = 91,
    FortressMisfortuneSilver = 92,
    FortressMisfortuneGold = 93,
    GoldieBronze = 94,
    GoldieSilver = 95,
    GoldieGold = 96,
    WitchCauldronBronze = 97,
    WitchCauldronSilver = 98,
    WitchCauldronGold = 99,
    PizzaFunghiBronze = 100,
    PizzaFunghiSilver = 101,
    PizzaFunghiGold = 102,
    GoblinLoveBronze = 103,
    GoblinLoveSilver = 104,
    GoblinLoveGold = 105,
    EquestrianismBronze = 106,
    EquestrianismSilver = 107,
    EquestrianismGold = 108,
    GuildDailyHeroBronze = 109,
    GuildDailyHeroSilver = 110,
    GuildDailyHeroGold = 111,
    PetPlushRumbleBronze = 112,
    PetPlushRumbleSilver = 113,
    PetPlushRumbleGold = 114,
    ManOfHonorBronze = 115,
    ManOfHonorSilver = 116,
    ManOfHonorGold = 117,
    PizzaFunghi2Bronze = 118,
    PizzaFunghi2Silver = 119,
    PizzaFunghi2Gold = 120,

    #[default]
    Unknown = -1,
}

/// The progress of a player towards all the deeds of glory
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Deeds {
    /// For every deed trigger level, whether or not the player has unlocked
    /// it already
    pub unlocked: EnumMap<DeedType, bool>,
}

impl Deeds {
    /// Checks if the given deed trigger level has been unlocked
    #[must_use]
    pub fn is_unlocked(&self, typ: DeedType) -> bool {
        self.unlocked[typ]
    }

    /// Parses the `deeds` response value. The server sends one flag per deed
    /// trigger level in `DeedType` order, but no slot for `DeedType::None`.
    /// The first value therefore belongs to `DeedType::SoulEaterBronze`
    pub(crate) fn update(&mut self, data: &[i64]) {
        for (deed_id, raw) in (1..).zip(data) {
            match DeedType::from_i64(deed_id) {
                Some(typ) => self.unlocked[typ] = *raw != 0,
                None if *raw != 0 => {
                    warn!("Unknown deed state for id {deed_id}: {raw}");
                }
                None => {}
            }
        }
    }

    /// Parses the `newdeeds` response value. The server sends this whenever
    /// playing unlocked one or more deed trigger levels. The value contains
    /// the ids of the `DeedType`s that were unlocked and the sub key contains
    /// the amount of them.
    pub(crate) fn update_new(&mut self, data: &[i64]) {
        for id in data {
            match DeedType::from_i64(*id) {
                Some(typ) => self.unlocked[typ] = true,
                None => warn!("Unknown new deed id: {id}"),
            }
        }
    }
}
