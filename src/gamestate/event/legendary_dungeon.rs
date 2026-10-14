use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{
    gamestate::{Item, Result, ServerTime},
    response::Vals,
};

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
    /// Updates the event timing from an `iadungeontime` response:
    /// `[theme, start, end, close]`.
    pub(crate) fn update_time(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.theme = data.enum_opt(0);
        self.start = data.server_time(1, server_time)?;
        self.end = data.server_time(2, server_time)?;
        self.close = data.server_time(3, server_time)?;
        Ok(())
    }

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
    /// Updates the legendary dungeon state from an `iadungeon` response.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0, "random id or session id");
        self.health_status = data.i64(1)?;

        self.current_hp = data.i64(2)?;
        self.pre_battle_hp = data.i64(3)?;
        self.max_hp = data.i64(4)?;

        for (pos, v) in self.blessings.iter_mut().enumerate() {
            let s = data.u32_or(11 + pos, 0);
            *v = DungeonEffect::parse(
                data.i64_or(5 + pos, 0),
                s / 10_000,
                data.u32_or(42 + pos, 0),
                s % 10_000,
            );
        }
        for (pos, v) in self.curses.iter_mut().enumerate() {
            let s_pos = match pos {
                0 => 14,
                1 => 40,
                _ => 41,
            };
            let s = data.u32_or(s_pos, 0);

            *v = DungeonEffect::parse(
                data.i64_or(8 + pos, 0),
                s / 10_000,
                data.u32_or(45 + pos, 0),
                s % 10_000,
            );
        }

        self.stage = data.enum_or(15, LegendaryDungeonStage::Unknown);

        data.ignore(16, "gem count");

        self.current_floor = data.u32_or(17, 0);
        self.max_floor = data.u32_or(18, 0);

        if self.stage == LegendaryDungeonStage::DoorSelect {
            for (pos, v) in self.doors.iter_mut().enumerate() {
                v.typ = data.enum_or(19 + pos, DoorType::Unknown);
                v.trap = data.enum_opt(25 + pos);
            }
        } else {
            self.room_type = data.enum_or(19, RoomType::Unknown);
            data.ignore(
                20,
                "door type for second door when not in door select",
            );
            data.ignore(25..27, "door traps when not in door select");
        }

        data.ignore(21, "unknown server value");
        let raw_enc = data.i64_or(22, 999);
        self.encounter = RoomEncounter::parse(raw_enc);
        data.ignore(23..25, "unknown server values");

        data.ignore(27..39, "moved server values");

        self.keys = data.u32_or(39, 0);

        data.ignore(48, "unknown server value");

        Ok(())
    }

    /// Updates pending items that must be collected or chosen between.
    pub(crate) fn update_pending_items(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.pending_items.clear();
        let amount = data.usize(0)?;
        if amount == 0 {
            return Ok(());
        }
        for chunk in data.window(1..data.len()).chunks(19) {
            if let Some(item) = Item::parse(&chunk, server_time)? {
                self.pending_items.push(item);
            } else {
                log::warn!("Could not parse pending ld item");
            }
        }
        Ok(())
    }

    /// Updates merchant offers from an `iamerchant` response of repeating
    /// 3-value chunks: `[effect_type, max_uses_and_strength, price]`.
    pub(crate) fn update_merchant(&mut self, data: &Vals<'_>) -> Result<()> {
        self.merchant_offers = data
            .chunks(3)
            .filter_map(|chunk| MerchantOffer::parse(&chunk).transpose())
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    /// Updates active and available gems of fate from an `iadungeonsoulstones`
    /// response of repeating 6-value chunks.
    ///
    /// The first 3 chunks are active gems; any remaining chunks are available
    /// gems to choose from.
    pub(crate) fn update_soul_stones(&mut self, data: &Vals<'_>) -> Result<()> {
        let mut chunks = data.chunks(6);
        self.active_gems = chunks
            .by_ref()
            .take(3)
            .filter_map(|chunk| GemOfFate::parse(&chunk).transpose())
            .collect::<Result<Vec<_>>>()?;

        self.available_gems = chunks
            .filter_map(|chunk| GemOfFate::parse(&chunk).transpose())
            .collect::<Result<Vec<_>>>()?;
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
    /// Parses a room encounter identifier from its raw integer value.
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
/// An active blessing or curse effect in the legendary dungeon
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
    /// Parses a dungeon effect from raw components, returning `None` if `typ <=
    /// 0`.
    pub(crate) fn parse(
        typ: i64,
        remaining: u32,
        max_uses: u32,
        strength: u32,
    ) -> Option<Self> {
        if typ <= 0 {
            return None;
        }
        let typ: DungeonEffectType =
            FromPrimitive::from_i64(typ).unwrap_or_default();

        Some(DungeonEffect {
            typ,
            remaining_uses: remaining,
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
    /// Parses the stats of the current run from
    /// `[items_found, epics_found, keys_found, silver_found, attempts]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(Stats {
            items_found: data.u32(0)?,
            epics_found: data.u32(1)?,
            keys_found: data.u32(2)?,
            silver_found: data.u64(3)?,
            attempts: data.u32(4)?,
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
    /// Parses a single merchant offer from a 3-element chunk:
    /// `[effect_type, max_uses_and_strength, price]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Option<Self>> {
        let raw_typ = data.i64(0)?;
        if raw_typ == 0 {
            data.ignore(1..3, "empty merchant offer");
            return Ok(None);
        }

        let typ: DungeonEffectType =
            data.enum_or(0, DungeonEffectType::Unknown);
        let s = data.u32(1)?;
        let price = data.u32(2)?;

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
    /// Parses total stats across all runs from
    /// `[legendaries_found, attempts_best_run, enemies_defeated, epics_found,
    /// gold_found, ...]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        // Note: There is another value (5), but I can not figure out what it is
        if data.len() > 5 {
            data.ignore(5, "unknown total stats field");
        }
        Ok(TotalStats {
            legendaries_found: data.u32(0)?,
            attempts_best_run: data.u32(1)?,
            enemies_defeated: data.u32(2)?,
            epics_found: data.u32(3)?,
            gold_found: data.u64(4)?,
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
    /// Parses a single gem of fate from a 6-element chunk:
    /// `[type, advantage, advantage_power, disadvantage, disadvantage_power,
    /// disadvantage_effect]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Option<GemOfFate>> {
        let raw_typ = data.i64(0)?;
        if raw_typ == 0 {
            data.ignore(1..6, "empty gem slot");
            return Ok(None);
        }
        Ok(Some(Self {
            typ: data.enum_or(0, GemOfFateType::Unknown),
            advantage: data.enum_opt(1),
            advantage_pwr: data.i64(2)?,
            disadvantage: data.enum_opt(3),
            disadvantage_pwr: data.i64(4)?,
            disadvantage_effect: data.enum_opt(5),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_legendary_dungeon_event_update_time() {
        let raw = "4/1781884800/1782676800/1782763200";
        let data = Vals::from(raw);
        let mut event = LegendaryDungeonEvent::default();
        event.update_time(&data, ServerTime::default()).unwrap();
        assert_eq!(
            event.theme,
            Some(LegendaryDungeonEventTheme::ShadyBirthdayBash)
        );
        assert!(event.start.is_some());
        assert!(event.end.is_some());
        assert!(event.close.is_some());
        assert_all_read(&data);
    }

    #[test]
    fn test_legendary_dungeon_update_door_select() {
        let raw = "718719374/2/1000/1000/1000/1/0/0/101/0/0/30050/0/0/20010/1/\
                   0/5/10/1/2/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/3/0/0/5/0/0/\
                   4/0/0/0";
        let data = Vals::from(raw);
        let mut ld = LegendaryDungeon::default();
        ld.update(&data).unwrap();
        assert_eq!(ld.health_status, 2);
        assert_eq!(ld.current_hp, 1000);
        assert_eq!(ld.pre_battle_hp, 1000);
        assert_eq!(ld.max_hp, 1000);
        assert_eq!(
            ld.blessings[0],
            Some(DungeonEffect {
                typ: DungeonEffectType::Raider,
                remaining_uses: 3,
                max_uses: 5,
                strength: 50,
            })
        );
        assert_eq!(ld.blessings[1], None);
        assert_eq!(
            ld.curses[0],
            Some(DungeonEffect {
                typ: DungeonEffectType::BrokenArmor,
                remaining_uses: 2,
                max_uses: 4,
                strength: 10,
            })
        );
        assert_eq!(ld.stage, LegendaryDungeonStage::DoorSelect);
        assert_eq!(ld.current_floor, 5);
        assert_eq!(ld.max_floor, 10);
        assert_eq!(ld.doors[0].typ, DoorType::Monster1);
        assert_eq!(ld.doors[0].trap, Some(DoorTrap::PoisonedDaggers));
        assert_eq!(ld.doors[1].typ, DoorType::Monster2);
        assert_eq!(ld.doors[1].trap, None);
        assert_eq!(ld.encounter, RoomEncounter::BronzeChest);
        assert_eq!(ld.keys, 3);
        assert_all_read(&data);
    }

    #[test]
    fn test_legendary_dungeon_update_room_entered() {
        let raw = "718719374/2/950/1000/1000/0/0/0/0/0/0/0/0/0/0/10/0/6/10/\
                   100/0/0/100/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/2/0/0/0/0/0/0/\
                   0/0/0";
        let data = Vals::from(raw);
        let mut ld = LegendaryDungeon::default();
        ld.update(&data).unwrap();
        assert_eq!(ld.stage, LegendaryDungeonStage::RoomEntered);
        assert_eq!(ld.room_type, RoomType::Encounter);
        assert_eq!(ld.encounter, RoomEncounter::Crate1);
        assert_eq!(ld.keys, 2);
        assert_all_read(&data);
    }

    #[test]
    fn test_stats_parse() {
        let raw = "10/2/5/50000/3";
        let data = Vals::from(raw);
        let stats = Stats::parse(&data).unwrap();
        assert_eq!(stats.items_found, 10);
        assert_eq!(stats.epics_found, 2);
        assert_eq!(stats.keys_found, 5);
        assert_eq!(stats.silver_found, 50000);
        assert_eq!(stats.attempts, 3);
        assert_all_read(&data);
    }

    #[test]
    fn test_total_stats_parse() {
        let raw = "5/20/150/12/1000000/0";
        let data = Vals::from(raw);
        let total = TotalStats::parse(&data).unwrap();
        assert_eq!(total.legendaries_found, 5);
        assert_eq!(total.attempts_best_run, 20);
        assert_eq!(total.enemies_defeated, 150);
        assert_eq!(total.epics_found, 12);
        assert_eq!(total.gold_found, 1_000_000);
        assert_all_read(&data);

        // Also test 5-element format
        let raw5 = "1/2/3/4/5";
        let data5 = Vals::from(raw5);
        let total5 = TotalStats::parse(&data5).unwrap();
        assert_eq!(total5.legendaries_found, 1);
        assert_all_read(&data5);
    }

    #[test]
    fn test_merchant_offer_parse() {
        let raw = "1/20050/10";
        let data = Vals::from(raw);
        let offer = MerchantOffer::parse(&data).unwrap().unwrap();
        assert_eq!(offer.typ, DungeonEffectType::Raider);
        assert_eq!(offer.max_uses, 2);
        assert_eq!(offer.strength, 50);
        assert_eq!(offer.keys, 10);
        assert_all_read(&data);

        let empty = Vals::from("0/0/0");
        assert!(MerchantOffer::parse(&empty).unwrap().is_none());
        assert_all_read(&empty);
    }

    #[test]
    fn test_update_merchant() {
        let raw = "1/20050/10/0/0/0/101/10020/5";
        let data = Vals::from(raw);
        let mut ld = LegendaryDungeon::default();
        ld.update_merchant(&data).unwrap();
        assert_eq!(ld.merchant_offers.len(), 2);
        assert_eq!(ld.merchant_offers[0].typ, DungeonEffectType::Raider);
        assert_eq!(ld.merchant_offers[1].typ, DungeonEffectType::BrokenArmor);
        assert_all_read(&data);
    }

    #[test]
    fn test_gem_of_fate_parse() {
        let raw = "1/1/10/30/5/1";
        let data = Vals::from(raw);
        let gem = GemOfFate::parse(&data).unwrap().unwrap();
        assert_eq!(gem.typ, GemOfFateType::EyeOfTheBull);
        assert_eq!(gem.advantage, Some(GemOfFateEffect::ChanceOfKeys));
        assert_eq!(gem.advantage_pwr, 10);
        assert_eq!(
            gem.disadvantage,
            Some(GemOfFateEffect::DurationOfBlessings)
        );
        assert_eq!(gem.disadvantage_pwr, 5);
        assert_eq!(
            gem.disadvantage_effect,
            Some(GemOfFateSpecialDisadvantage::WeakerMonstersSpawn)
        );
        assert_all_read(&data);

        let empty = Vals::from("0/0/0/0/0/0");
        assert!(GemOfFate::parse(&empty).unwrap().is_none());
        assert_all_read(&empty);
    }

    #[test]
    fn test_update_soul_stones() {
        let raw = "1/1/10/30/5/1/2/1/10/30/5/1/0/0/0/0/0/0/3/1/10/30/5/1";
        let data = Vals::from(raw);
        let mut ld = LegendaryDungeon::default();
        ld.update_soul_stones(&data).unwrap();
        assert_eq!(ld.active_gems.len(), 2);
        assert_eq!(ld.available_gems.len(), 1);
        assert_all_read(&data);
    }

    #[test]
    fn test_update_pending_items() {
        let empty_raw = "0";
        let empty_data = Vals::from(empty_raw);
        let mut ld = LegendaryDungeon::default();
        ld.update_pending_items(&empty_data, ServerTime::default())
            .unwrap();
        assert_eq!(ld.pending_items, Vec::new());
        assert_all_read(&empty_data);

        let item_raw =
            "1/1/0/0/1010/0/149/265/5/4/0/108/72/0/83940/10/0/0/45/0";
        let item_data = Vals::from(item_raw);
        ld.update_pending_items(&item_data, ServerTime::default())
            .unwrap();
        assert_eq!(ld.pending_items.len(), 1);
        assert_all_read(&item_data);
    }
}
