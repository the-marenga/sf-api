use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{error::SFError, gamestate::items::Item, misc::*};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the legendary dungeon event
pub struct LegendaryDungeonEvent {
    /// The theme of the current event
    pub theme: Option<LegendaryDungeonEventTheme>,
    /// The time after which we are allowed to interact with the legendary
    /// dungeons
    pub start: Option<DateTime<Local>>,
    /// The time up until which we are allowed to start new runs
    pub end: Option<DateTime<Local>>,
    /// The time at which the dungeon is expected to completely close.
    /// Interacting with the dungeon (at all) is not possible after this.
    pub close: Option<DateTime<Local>>,

    pub(crate) active: Option<LegendaryDungeon>,
}

impl LegendaryDungeonEvent {
    #[must_use]
    /// Returns the status of the legendary dungeon event. This is basically the
    /// screen, that you would be looking at in-game
    pub fn status(&self) -> LegendaryDungeonStatus<'_> {
        use LegendaryDungeonStage as Stage;
        use LegendaryDungeonStatus as Status;

        let now = Local::now();
        if self.start.is_none_or(|a| a > now) {
            return Status::Unavailable;
        }
        if self.close.is_none_or(|a| a < now) {
            return Status::Unavailable;
        }

        let Some(theme) = self
            .theme
            .filter(|a| !matches!(a, LegendaryDungeonEventTheme::Unknown))
        else {
            return Status::Unavailable;
        };

        let Some(active) = &self.active else {
            return if self.end.is_some_and(|a| a > now) {
                Status::NotEntered(theme)
            } else {
                Status::Unavailable
            };
        };

        if !active.pending_items.is_empty() {
            return Status::TakeItem {
                dungeon: active,
                items: &active.pending_items,
            };
        }

        let room_status = |status| Status::Room {
            dungeon: active,
            status,
            encounter: active.encounter,
            typ: active.room_type,
        };

        match active.stage {
            Stage::NotEntered => Status::NotEntered(theme),
            Stage::DoorSelect => Status::DoorSelect {
                dungeon: active,
                doors: &active.doors,
            },
            Stage::RoomSpecial if active.room_type == RoomType::BossRoom => {
                Status::PickGem {
                    dungeon: active,
                    available_gems: &active.available_gems,
                }
            }
            #[allow(clippy::pedantic)]
            Stage::Healing => {
                let started = active.healing_start.unwrap_or_default();
                let now = Local::now();
                let elapsed = now - started;
                let elapsed_minuted = elapsed.num_minutes() as f64;

                let heal_per_day = 100.0;
                let heal_per_hour = heal_per_day / 24.0;
                let heal_per_minute = heal_per_hour / 60.0;

                let healed = elapsed_minuted * heal_per_minute;
                let current_health_percent = healed.clamp(0.0, 100.0) as u8;

                Status::Healing {
                    dungeon: active,
                    started,
                    current_health_percent,
                }
            }
            Stage::RoomEntered => room_status(RoomStatus::Entered),
            Stage::RoomInteracted => room_status(RoomStatus::Interacted),
            Stage::RoomSpecial => room_status(RoomStatus::Special),
            Stage::RoomFinished => room_status(RoomStatus::Finished),
            Stage::Unknown => Status::Unknown,
            Stage::Finished => Status::Ended(&active.total_stats),
        }
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The theme of the legendary dungeon event
pub enum LegendaryDungeonEventTheme {
    /// Diabolical Company Party theme
    DiabolicalCompanyParty = 1,
    /// Lord of the Things theme
    LordOfTheThings = 2,
    /// Fantastic Legendaries theme
    FantasticLegendaries = 3,
    /// Shady Birthday Bash theme
    ShadyBirthdayBash = 4,
    /// Massive Winter Spectacle theme
    MassiveWinterSpectacle = 5,
    /// Abyss of Madness theme
    AbyssOfMadness = 6,
    /// Hunt for Blazing Easter Egg theme
    HuntForBlazingEasterEgg = 7,
    /// Vile Vacation theme
    VileVacation = 8,

    /// A theme that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The state of a legendary dungeon run
pub struct LegendaryDungeon {
    /// Statistics for the current run
    pub stats: Stats,
    /// Total statistics across all runs in this event
    pub total_stats: TotalStats,

    /// The hp you currently have
    pub current_hp: i64,
    /// Any action, that reduces hp will immediately update `current_hp`. In
    /// order for the game to properly transition from your old hp to the
    /// current hp (visually), this here will contain your previous hp from
    /// befor you took the action
    pub pre_battle_hp: i64,
    /// The hp you started the dungeon with
    pub max_hp: i64,

    /// The blessings currently active (max 3)
    pub blessings: [Option<DungeonEffect>; 3],
    /// The curses currently active (max 3)
    pub curses: [Option<DungeonEffect>; 3],

    pub(crate) stage: LegendaryDungeonStage,

    /// The current floor you are on
    pub current_floor: u32,
    /// The highest floor you have reached in this run
    pub max_floor: u32,
    /// The amount of keys you have available to unlock doors
    pub keys: u32,

    /// The amount of mushrooms you would have to spend to heal 20% of your
    /// health
    pub heal_quarter_cost: u32,
    /// The effects the merchant is currently trying to sell you
    pub merchant_offers: Vec<MerchantOffer>,
    /// The gems available to choose from after defeating the boss
    pub active_gems: Vec<GemOfFate>,

    /// The doors that you can pick between when in the `DoorSelect` stage
    pub(crate) doors: [Door; 2],
    pub(crate) room_type: RoomType,
    /// The thing you currently have in the room with you
    pub(crate) encounter: RoomEncounter,
    /// Items, that must be collected/chosen between before you can continue
    pub(crate) pending_items: Vec<Item>,
    /// The gems available to choose from after defeating the boss
    pub(crate) available_gems: Vec<GemOfFate>,

    // 2 = alive
    // 1 = ?
    // 0 = dead (healing)
    health_status: i64,

    /// The time at which the healing process started after dropping hp to 0
    pub(crate) healing_start: Option<DateTime<Local>>,
}

impl LegendaryDungeon {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        // [00] 718719374 <= Some sort of random id?
        self.health_status = data.cget(1, "ld unknown")?;

        self.current_hp = data.cget(2, "ld current hp")?;
        self.pre_battle_hp = data.cget(3, "ld pre hp")?;
        self.max_hp = data.cget(4, "ld max hp")?;

        for (pos, v) in self.blessings.iter_mut().enumerate() {
            let s = data.csiget(11 + pos, "ld blessing rem", 0)?;
            *v = DungeonEffect::parse(
                data.csiget(5 + pos, "ld blessing typ", 0)?,
                s / 10_000,
                data.csiget(42 + pos, "ld blessing max", 0)?,
                s % 10_000,
            );
        }
        for (pos, v) in self.curses.iter_mut().enumerate() {
            let s_pos = match pos {
                0 => 14,
                1 => 40,
                _ => 41,
            };
            let s = data.csiget(s_pos, "ld blessing rem", 0)?;

            *v = DungeonEffect::parse(
                data.csiget(8 + pos, "ld blessing typ", 0)?,
                s / 10_000,
                data.csiget(45 + pos, "ld blessing max", 0)?,
                s % 10_000,
            );
        }

        self.stage =
            data.cfpget(15, "dungeon stage", |a| a)?.unwrap_or_default();

        // 16 => gem count

        self.current_floor = data.csiget(17, "ld floor", 0)?;
        self.max_floor = data.csiget(18, "ld max floor", 0)?;

        if self.stage == LegendaryDungeonStage::DoorSelect {
            for (pos, v) in self.doors.iter_mut().enumerate() {
                v.typ = data
                    .cfpget(19 + pos, "ld door typ", |a| a)?
                    .unwrap_or_default();

                let raw_trap = data.cget(25 + pos, "ld door trap")?;
                v.trap = match raw_trap {
                    0 => None,
                    x => FromPrimitive::from_i64(x),
                }
            }
        } else {
            self.room_type =
                data.cfpget(19, "ld room type", |a| a)?.unwrap_or_default();
        }

        let raw_enc = data.csiget(22, "ld encounter", 999)?;
        self.encounter = RoomEncounter::parse(raw_enc);

        // 27..= 38 has moved

        self.keys = data.csiget(39, "ld keys", 0)?;

        // Unknown:
        // 21, 23, 24, 40, 41, 48;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The current status of the legendary dungeon
pub enum LegendaryDungeonStatus<'a> {
    /// The legendary dungeon is not open, so you can not interact with it in
    /// any way, shape or form
    Unavailable,
    /// You have not yet entered the dungeon. Start the dungeon by sending a
    /// `LegendaryDungeonStart` command
    NotEntered(LegendaryDungeonEventTheme),
    /// The event has ended, so you are not allowed to start another attempt,
    /// but you may look at your stats
    Ended(&'a TotalStats),
    /// You are in the door select screen and must either pick the left, or
    /// right door. You do so with the `LegendaryDungeonPickDoor` command with
    /// the index of the door
    DoorSelect {
        /// The legendary dungeon state
        dungeon: &'a LegendaryDungeon,
        /// The two doors available to pick from
        doors: &'a [Door; 2],
    },
    /// You have defeated the dungeon boss and must pick one of the offered
    /// gems. You do so with the `IADungeonSelectSoulStone` command and the
    /// type of the gem you want
    PickGem {
        /// The legendary dungeon state
        dungeon: &'a LegendaryDungeon,
        /// The gems available to choose from
        available_gems: &'a [GemOfFate],
    },
    /// We are currently healing. Wait until you can continue.
    Healing {
        /// The legendary dungeon state
        dungeon: &'a LegendaryDungeon,
        /// The time at which we started to heal (last time we died)
        started: DateTime<Local>,
        /// [0-100]. Will be updated based on the current `DateTime`, so this
        /// will change inbetween invocations
        current_health_percent: u8,
    },
    /// You are currently in a room
    Room {
        /// The legendary dungeon state
        dungeon: &'a LegendaryDungeon,
        /// The status of the room (entered, interacted, etc.)
        status: RoomStatus,
        /// The encounter in the room
        encounter: RoomEncounter,
        /// The type of room
        typ: RoomType,
    },
    /// You have items that need to be taken
    TakeItem {
        /// The legendary dungeon state
        dungeon: &'a LegendaryDungeon,
        /// The items available to take
        items: &'a [Item],
    },
    /// The dungeon is in a state, that has not been anticipated. Your best bet
    /// is to send a `LegendaryDungeonInteract` with a value of:
    /// [0,20,40,50,51,60,70] If you get this status, please report it
    Unknown,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The current stage of the legendary dungeon run
pub enum LegendaryDungeonStage {
    /// The dungeon has not been entered yet
    NotEntered = 0,

    /// The player is currently selecting a door
    DoorSelect = 1,

    /// A room has been entered
    RoomEntered = 10,
    /// The player has interacted with the room
    RoomInteracted = 11,
    /// A special room event is occurring
    RoomSpecial = 12,

    /// The room has been finished
    RoomFinished = 100,
    /// The player is currently healing
    Healing = 101,
    /// The dungeon run has finished
    Finished = 102,

    /// A stage that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of room in the legendary dungeon
pub enum RoomType {
    /// A generic room with nothing special
    Generic = 1,
    /// A room with a mini-boss
    BossRoom = 4,
    /// The room with the final boss of the dungeon
    FinalBossRoom = 5,

    /// A room with a standard encounter (e.g. chest, crate)
    Encounter = 100,
    /// An empty room
    Empty = 200,
    /// The base version of the fountain of life heals a part of a character's
    /// life energy.
    FountainOfLife = 301,
    /// Immediately triggers the game to interact. No idea why, or what this is
    HoleInTheFloor = 302,
    /// The rocks must be cleared away to progress in the Legendary Dungeon. As
    /// a reward, stones are added to the fortress storage.
    PileOfRocks = 303,
    /// You have no choice; the lava must be crossed to progress in the
    /// Legendary Dungeon. Unfortunately, this also means a loss of a part of
    /// the character's life energy.
    TheFloorIsLava = 304,
    /// The dungeon narrator offers you a cup of tea. You can decide whether to
    /// drink the tea or not. If you drink the tea, it heals a part of the
    /// character's life energy, and you receive a blessing. If you do not
    /// drink the tea, you leave the room without any consequences.
    DungeonNarrator = 305,
    /// The room slowly fills with water. If you do not leave within ten
    /// seconds, you will drown and die.
    // 50 => continue
    // 51 => death
    FloodedRoom = 306,
    /// If you throw a gold coin into the wishing well, you will receive either
    /// an item or a blessing.
    WishingWell = 307,
    /// The gambler challenges you to rock-paper-scissors.
    /// If you avoid the challenge, you leave the room without any effects.
    /// If you dare to compete against the gambler, you choose either rock,
    /// paper, or scissors and wait to see what the gambler chooses.
    /// If you defeat the gambler, you receive a blessing.
    /// If the gambler wins, you receive a curse and lose a part of your life
    /// energy.
    /// If it's a draw, nothing happens.
    RockPaperScissors = 308,
    /// If you have wondered where the items go that you threw into the arcane
    /// toilet, here you find the answer.
    /// If you search through the broth, you receive an item.
    /// Too disgusting? The sewers can be left without any effects.
    Sewers = 309,
    /// The zombie with the lantern challenges you to a fight. You can accept
    /// the fight or try to flee.
    UndeadFiend = 310,
    /// An epic item is hidden in the locker. It's worth taking a look.
    LockerRoom = 311,
    /// The unlocked sarcophagus can be opened without a key. It contains gold.
    UnlockedSarcophagus = 312,
    /// If you defeat the Valaraukar, you lose a part of your hit points, but
    /// in return, you also receive the blessing "Road to Recovery".
    /// Cowardly heroes try to dodge the Valaraukar.
    Valaraukar = 313,
    /// The wood must be cleared away to progress in the Legendary Dungeon. As a
    /// reward, wood is added to the fortress storage.
    PileOfWood = 314,
    /// Buy blessings with keys
    KeyMasterShop = 315,
    /// The wheel of fortune can be spun. With some luck, you receive a reward
    /// (gold, blessing), but with bad luck, you can lose keys or receive a
    /// curse. You can leave the room without spinning the wheel, without
    /// having to worry about any consequences.
    WheelOfFortune = 316,
    /// Keys can be found in the spider web. However, it's also possible to get
    /// bitten by the spider and get poisoned (curse). If you don't want to
    /// take any risks, you can simply leave the room. There are three
    /// variations of the spider web:
    /// - Only the spider legs are visible: high chance for a key, low chance of
    ///   a spider bite.
    /// - The spider head is visible: equal chance for two keys or a spider
    ///   bite.
    /// - The entire spider is visible: low chance for 5 keys, high chance of a
    ///   spider bite.
    SpiderWeb = 317,

    // 318 unused?
    /// You can try to defeat the monster or dodge and take flight.
    BetaRoom = 319,
    /// If you defeat the flying tube, you are rewarded with ten lucky coins.
    FlyingTube = 320,
    /// If you click on the soul bath, you get credited souls in the underworld.
    SoulBath = 321,
    /// The arcane splinters can be collected and then used at the blacksmith.
    ArcaneSplintersCave = 322,
    /// The key to failure shop master offers curses for purchase. This might
    /// not sound very appealing at first. However, you receive keys if you take
    /// up the offer. If you're not interested, you can simply leave the
    /// store.
    KeyToFailureShop = 323,
    /// If you subject yourself to the torture rack, you receive a blessing but
    /// also lose a part of the character's life energy at the same time.
    /// The rainbow room can be left without any consequences.
    RainbowRoom = 324,
    /// You lose a part of your life energy in the fight. If the fight is won,
    /// you receive life energy as a reward, where the gain in life energy
    /// exceeds the loss from the fight. Lucky you!
    /// The room can be left without any consequences.
    PigRoom = 325,
    /// If you click on the offer board, you receive an item that can be epic.
    /// You can also simply leave the room without anything happening.
    AuctionHouse = 326,
    /// A hostile cat is blocking your way
    MonsterCat1 = 327,
    /// A hostile cat is blocking your way
    MonsterCat2 = 328,
    /// A room with weapons and armor
    Armory = 329,
    /// A room type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The status of the current room
pub enum RoomStatus {
    /// The room has been entered
    Entered,
    /// The player has interacted with the room content
    Interacted,
    /// A special event is happening in the room
    Special,
    /// The room has been cleared
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The encounter present in a room
pub enum RoomEncounter {
    /// A bronze chest with basic loot
    BronzeChest,
    /// A silver chest with better loot
    SilverChest,
    /// An epic chest with great loot
    EpicChest,
    /// A basic crate
    Crate1,
    /// A medium crate
    Crate2,
    /// A large crate
    Crate3,

    /// Pretty sure this is exclusively the dead (lootable) one
    WarriorSkeleton,
    /// The thing that transforms into an enemy
    MageSkeleton,
    /// A barrel that can be smashed
    Barrel,

    /// A chest that is actually a mimic
    MimicChest,
    /// A chest that requires a sacrifice
    SacrificialChest,
    /// A chest that is cursed
    CurseChest,
    /// The price chest for completing the trial
    PrizeChest,
    /// A chest that is sated (requires something to open)
    SatedChest,

    /// A monster encounter with the given ID
    Monster(u16),
    /// An encounter type that has not yet been implemented
    #[default]
    Unknown,
}

impl RoomEncounter {
    pub(crate) fn parse(val: i64) -> RoomEncounter {
        match val {
            0 => RoomEncounter::BronzeChest,
            1 => RoomEncounter::SilverChest,
            2 => RoomEncounter::EpicChest,
            100 => RoomEncounter::Crate1,
            101 => RoomEncounter::Crate2,
            102 => RoomEncounter::Crate3,
            300 => RoomEncounter::MageSkeleton,
            301 => RoomEncounter::WarriorSkeleton,
            400 => RoomEncounter::Barrel,
            500 => RoomEncounter::MimicChest,
            600 => RoomEncounter::SacrificialChest,
            601 => RoomEncounter::CurseChest,
            602 => RoomEncounter::PrizeChest,
            603 => RoomEncounter::SatedChest,
            x if x.is_negative() => {
                RoomEncounter::Monster(x.abs().try_into().unwrap_or_default())
            }
            _ => {
                log::warn!("Unknown room encounter: {val}");
                RoomEncounter::Unknown
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A door in the legendary dungeon
pub struct Door {
    /// The type of door
    pub typ: DoorType,
    /// The trap that might be present on the door
    pub trap: Option<DoorTrap>,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of door you can encounter in the legendary dungeon
pub enum DoorType {
    /// A door with a monster
    Monster1 = 1,
    /// A door with a monster
    Monster2 = 2,
    /// A door with a monster
    Monster3 = 3,
    /// The door to the boss
    Boss1 = 4,
    /// The door to the boss
    Boss2 = 5,

    /// Bricked-up doors block the way. Here, passage is denied, and one is
    /// forced to go through the other door.
    Blocked = 1000,
    /// Behind a mysterious door, there could be an empty room, an enemy room,
    /// or an interaction room.
    MysteryDoor = 1001,
    /// A door that requires a key to open
    LockedDoor = 1002,
    /// An already open door
    OpenDoor = 1003,
    /// Behind an epic door, there is an epic chest. To open an epic door, a key
    /// is required.
    EpicDoor = 1004,
    /// A door that requires two keys to open
    DoubleLockedDoor = 1005,
    /// Golden rooms can appear behind golden doors.
    GoldenDoor = 1006,
    /// Passing through a sacrificial door, you will lose a part of your life
    /// energy. Behind this door, there is always a sacrificial chest.
    SacrificialDoor = 1007,
    /// Opening a cursed door results in receiving a curse. Normally, a cursed
    /// chest is found behind a cursed door
    CursedDoor = 1008,
    /// A door leading to the key master's shop
    KeyMasterShop = 1009,
    /// A door that will give you a blessing when entering
    BlessingDoor = 1010,
    /// To open this door, you need to spin the wheel of fortune. No key is
    /// required.
    // A random reward is received (blessing, gold, lucky coins, hourglasses,
    // or mushrooms). If unlucky, a curse is imposed. Behind a destiny
    // door, there can be an empty room, a monster room, or an interaction
    // room. In an interaction room, there is a wooden box, a chest, a corpse,
    // or a barrel.
    #[doc(alias = "Destiny")]
    Wheel = 1011,
    /// A hungry door that requires a certain amount of wood to open
    /// When you open the hungry door, you enter a room where a sated chest can
    /// be found. In the chest, you find resources (wood, stone, souls, metal,
    /// arcane splinters, or hourglasses of impatience), but not the resource
    /// previously fed.
    Wood = 1012,
    /// A hungry door that requires a certain amount of stone to open
    /// When you open the hungry door, you enter a room where a sated chest can
    /// be found. In the chest, you find resources (wood, stone, souls, metal,
    /// arcane splinters, or hourglasses of impatience), but not the resource
    /// previously fed.
    Stone = 1013,
    /// A hungry door that requires a certain amount of sould to open
    /// When you open the hungry door, you enter a room where a sated chest can
    /// be found. In the chest, you find resources (wood, stone, souls, metal,
    /// arcane splinters, or hourglasses of impatience), but not the resource
    /// previously fed.
    Souls = 1014,
    /// A hungry door that requires a certain amount of metal to open
    /// When you open the hungry door, you enter a room where a sated chest can
    /// be found. In the chest, you find resources (wood, stone, souls, metal,
    /// arcane splinters, or hourglasses of impatience), but not the resource
    /// previously fed.
    Metal = 1015,
    /// A hungry door that requires a certain amount of arcane to open
    /// When you open the hungry door, you enter a room where a sated chest can
    /// be found. In the chest, you find resources (wood, stone, souls, metal,
    /// arcane splinters, or hourglasses of impatience), but not the resource
    /// previously fed.
    Arcane = 1016,
    /// A hungry door that requires a certain amount of quicksand Hourglasses to
    /// open When you open the hungry door, you enter a room where a sated
    /// chest can be found. In the chest, you find resources (wood, stone,
    /// souls, metal, arcane splinters, or hourglasses of impatience), but
    /// not the resource previously fed.
    QuicksandGlasses = 1017,
    /// The first trial room
    TrialRoom1 = 1018,
    /// The second trial room
    TrialRoom2 = 1019,
    /// The third trial room
    TrialRoom3 = 1020,
    /// The fourth trial room
    TrialRoom4 = 1021,
    /// The fifth trial room
    TrialRoom5 = 1022,
    /// The exit of the trial rooms
    TrialRoomExit = 1023,

    /// A door type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A trap that can be encountered on a door
pub enum DoorTrap {
    /// Poisoned daggers trap. I think this gives a curse
    PoisonedDaggers = 1,
    /// A swinging axe trap
    SwingingAxe = 2,
    /// A paint bucket trap
    PaintBucket = 3,
    /// A bear trap
    BearTrap = 4,
    /// A guillotine trap
    Guillotine = 5,
    /// A hammer ambush trap
    HammerAmbush = 6,
    /// A trip wire trap
    TripWire = 7,
    /// A top spikes trap
    TopSpikes = 8,
    /// A shark trap
    Shark = 9,

    /// A trap type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DungeonEffect {
    /// The type this effect has
    pub typ: DungeonEffectType,
    /// The amount of rooms, or uses this effect is still active for
    pub remaining_uses: u32,
    /// The amount of rooms, or uses this effect will be active for after you
    /// get it (always >= remaining)
    pub max_uses: u32,
    /// The strength of this effect. I.e. 50 => chance to escape +50%
    pub strength: u32,
}

impl DungeonEffect {
    pub(crate) fn parse(
        typ: i64,
        remaining: i64,
        max_uses: i64,
        strength: i64,
    ) -> Option<Self> {
        if typ <= 0 {
            return None;
        }
        let typ: DungeonEffectType =
            FromPrimitive::from_i64(typ).unwrap_or_default();

        let remaining_uses: u32 = remaining.try_into().unwrap_or(0);
        let max_uses: u32 = max_uses.try_into().unwrap_or(0);
        let strength: u32 = strength.try_into().unwrap_or(0);

        Some(DungeonEffect {
            typ,
            remaining_uses,
            max_uses,
            strength,
        })
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An effect, that you can get in the legendary dungeon
pub enum DungeonEffectType {
    /// More gold from chests
    Raider = 1,
    /// Kill monster in one hit
    OneHitWonder = 2,
    /// Better chance to escape
    EscapeAssistant = 3,
    /// Disarm the next X traps
    DisarmTraps = 4,
    /// Open the next X doors without keys
    LockPick = 5,
    /// 50% chance for 2 keys
    KeyMoment = 6,
    /// Heal X% of life immediately
    ElixirOfLife = 7,
    /// Heal X% per room
    RoadToRecovery = 8,

    /// Enemy deals more damage
    BrokenArmor = 101,
    /// Receive X% damage each room
    Poisoned = 102,
    /// Lower chance to escape
    Panderous = 103,
    /// Less gold from chests
    GoldRushHangover = 104,
    /// Double key price
    HardLock = 105,

    /// A curse/blessing, that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Statistics for the current legendary dungeon run
pub struct Stats {
    /// The amount of items found during this run
    pub items_found: u32,
    /// The amount of epic items found during this run
    pub epics_found: u32,
    /// The amount of keys found during this run
    pub keys_found: u32,
    /// The amount of silver found during this run
    pub silver_found: u64,
    /// The amount of attempts made in this run
    pub attempts: u32,
}

impl Stats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(Stats {
            items_found: data.csiget(0, "ld item found", 0)?,
            epics_found: data.csiget(1, "ld epic found", 0)?,
            keys_found: data.csiget(2, "ld keys found", 0)?,
            silver_found: data.csiget(3, "ld silver found", 0)?,
            attempts: data.csiget(4, "ld attempts", 0)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An offer from the merchant in the legendary dungeon
pub struct MerchantOffer {
    /// The type of effect being offered
    pub typ: DungeonEffectType,
    /// The maximum number of uses for this effect
    pub max_uses: u32,
    /// The strength of the effect
    pub strength: u32,
    /// The amount of keys you pay, or get depending on curse/blessing
    pub keys: u32,
}

impl MerchantOffer {
    pub(crate) fn parse(data: &[i64]) -> Result<Option<Self>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        let typ: DungeonEffectType = data
            .cfpget(0, "ld merchant offer type", |a| a)?
            .unwrap_or_default();

        let s: u32 = data.csiget(1, "ld merchant effect", 0)?;
        let price = data.csiget(2, "ld merchant price", u32::MAX)?;
        Ok(Some(Self {
            typ,
            max_uses: s / 10_000,
            strength: s % 10_000,
            keys: price,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Total statistics across all runs in the legendary dungeon event
pub struct TotalStats {
    /// Total number of legendary items found
    pub legendaries_found: u32,
    /// Most attempts made in a single best run
    pub attempts_best_run: u32,
    /// Total number of enemies defeated
    pub enemies_defeated: u32,
    /// Total number of epic items found
    pub epics_found: u32,
    /// Total gold found
    pub gold_found: u64,
}

impl TotalStats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        // Note: There is another value (5), but I can not figure out what it is
        Ok(TotalStats {
            legendaries_found: data.csiget(0, "ld total legendaries", 0)?,
            attempts_best_run: data.csiget(1, "ld best attempts", 0)?,
            enemies_defeated: data.csiget(2, "ld enemies defeated", 0)?,
            epics_found: data.csiget(3, "ld total epics", 0)?,
            gold_found: data.csiget(4, "ld total gold", 0)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A gem of fate, which provides a permanent buff (or debuff) for the rest of
/// the legendary dungeon event
pub struct GemOfFate {
    /// The type of gem
    pub typ: GemOfFateType,
    /// The positive effect provided by the gem
    pub advantage: Option<GemOfFateEffect>,
    /// The power/value of the positive effect
    pub advantage_pwr: i64,
    /// The negative effect provided by the gem
    pub disadvantage: Option<GemOfFateEffect>,
    /// The power/value of the negative effect
    pub disadvantage_pwr: i64,
    /// A special disadvantage effect
    pub disadvantage_effect: Option<GemOfFateSpecialDisadvantage>,
}

impl GemOfFate {
    pub(crate) fn parse(data: &[i64]) -> Result<Option<GemOfFate>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        Ok(Some(Self {
            typ: data.cfpget(0, "ld gof typ", |a| a)?.unwrap_or_default(),
            advantage: data.cfpget(1, "ld gof adv", |a| a)?,
            advantage_pwr: data.cget(2, "ld gof dis val")?,
            disadvantage: data.cfpget(3, "ld gof dis", |a| a)?,
            disadvantage_pwr: data.cget(4, "ld gof dis val")?,
            disadvantage_effect: data.cfpget(5, "ld gof dis effect", |a| a)?,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of Gem of Fate
pub enum GemOfFateType {
    /// Eye of the Bull gem
    EyeOfTheBull = 1,
    /// Soul of the Rabbit gem
    SoulOfTheRabbit = 2,
    /// Boulder of Greed gem
    BoulderOfGreed = 3,
    /// Emerald of the Explorer gem
    EmeraldOfTheExplorer = 4,
    /// Pearl of the Masochist gem
    PearlOfTheMasochist = 5,
    /// Pendant of the Key Master gem
    PendantOfTheKeyMaster = 6,
    /// Pebble of Deceit gem
    PebbleOfDeceit = 7,
    /// Greasy Healing Stone gem
    GreasyHealingStone = 8,
    /// Spying Gem gem
    SpyingGem = 9,
    /// Lode Stone gem
    LodeStone = 10,
    /// Boulder of the Gambler gem
    BoulderOfTheGambler = 11,
    /// Old Sacrificial Stone gem
    OldSacrificialStone = 12,
    /// Blood Drop of Sacrifice gem
    BloodDropOfSacrifice = 13,
    /// Kidney Stone of Determination gem
    KidneyStoneOfDetermination = 14,
    /// Hope of the Thirsty One gem
    HopeOfTheThirstyOne = 15,
    /// Erratic Boulder of the Hip gem
    ErraticBoulderOfTheHip = 16,
    /// Saphire of the Misadventurer gem
    SaphireOfTheMisadventurer = 17,
    /// Cursed Moonstone gem
    CursedMoonstone = 18,
    /// Diamond of the Timetraveler gem
    DiamondOfTheTimetraveler = 19,
    /// Treasure of the Hero gem
    TreasureOfTheHero = 20,
    /// Crown Jewel of the Devil gem
    CrownJewelOfTheDevil = 21,
    /// Cursed Pearl gem
    CursedPearl = 22,
    /// Rusty Healing Stone gem
    RustyHealingStone = 23,

    /// A gem type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An effect provided by a Gem of Fate
pub enum GemOfFateEffect {
    // Key related effects
    /// Affects the chance of finding keys
    ChanceOfKeys = 1,

    // Escape effects
    /// Affects the chance of escaping from monsters
    EscapeChance = 10,
    /// Affects the damage taken when escaping
    DamageFromEscape = 11,
    /// Affects the chance of finding a key after escaping
    ChanceOfKeyAfterEscape = 12,
    /// Affects the chance of receiving a curse after escaping
    ChanceOfCurseAfterEscape = 13,

    // Blessings & Curses
    /// Affects the duration of blessings
    DurationOfBlessings = 30,
    /// Affects the duration of curses
    DurationOfCurses = 31,
    /// Affects the chance of receiving stronger curses
    ChanceOfStrongerCurses = 32,

    // Fights
    /// Affects the damage taken from monsters
    DamageFromMonsters = 40,
    /// Affects the chance of receiving a blessing after a fight
    ChanceOfBlessingAfterFight = 41,
    /// Affects the chance of receiving a curse after a fight
    ChanceOfCurseAfterFight = 42,

    // Barrels
    /// Affects the chance of finding blessings in barrels
    ChanceOfBlessingsInBarrels = 50,
    /// Affects the chance of finding better blessings in barrels
    ChanceOfBetterBlessingsInBarrels = 51,

    // Damage
    /// Affects the damage taken from sacrificial doors
    DamageFromSacDoors = 70,
    /// Affects the damage taken from opening chests
    DamageFromChests = 71,

    /// Affects the damage taken from traps
    DamageFromTraps = 90,

    /// Affects whether a blessing or curse is received after reviving
    BlessingOrCurseAfterRevive = 100,
    /// Affects the chance of finding blessings in barrels, chests, or corpses
    BlessingsInBarrelsChestsCorpses = 110,
    /// Affects the amount of healing received from blessings
    HealingFromBlessings = 130,

    /// An effect type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A special disadvantage effect provided by a Gem of Fate
pub enum GemOfFateSpecialDisadvantage {
    /// Weaker monsters spawn in the dungeon
    WeakerMonstersSpawn = 1,
    /// Stronger monsters spawn in the dungeon
    StrongerMonstersSpawn = 2,
    /// More traps spawn on doors
    MoreTrapsSpawn = 3,
    // 4-5?
    /// Sacrificial chests spawn behind closed doors
    SacChestsSpawnBehindClosedDoors = 6,

    /// More sacrificial doors appear
    MoreSacDoors = 8,
    /// Fewer sacrificial doors appear
    FewerSacDoors = 9,
    /// Cursed chests spawn behind closed doors
    CursedChestsSpawnBehindClosedDoors = 10,

    /// More cursed doors appear
    MoreCursedDoors = 12,
    /// Fewer cursed doors appear
    FewerCursedDoors = 13,

    // ??
    /// Affects the chance of epic doors appearing
    ChanceOfEpicDoors = 17,
    /// Affects the chance of unlocked doors appearing
    ChanceOfUnlockedDoors = 18,
    /// Affects the chance of double locked doors appearing
    ChanceOfDoubleLockedDoor = 19,

    /// More mysterious rooms appear
    MoreMysteriousRooms = 20,
    /// Fewer mysterious rooms appear
    FewerMysteriousRooms = 21,
    /// Every door will have at least one trap
    AlwaysOneTrap = 22,
    /// Every door will have at least one lock
    AlwaysOneLock = 23,
    /// Monsters are always present behind doors
    MonstersBehindDoors = 24,
    /// No more epic chests will spawn
    NoMoreEpicChests = 25,
    /// Traps will always inflict a curse
    TrapsInflictCurse = 26,

    /// A disadvantage type that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A choice in the rock-paper-scissors minigame
pub enum RPSChoice {
    /// Rock choice
    Rock = 90,
    /// Paper choice
    Paper = 91,
    /// Scissors choice
    Scissors = 92,
}
