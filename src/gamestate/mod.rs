/// Arena fights against other players.
pub mod arena;
/// Character state, attributes, inventory, and equipment.
pub mod character;
/// Light and shadow dungeons, dungeon tower, and portal.
pub mod dungeons;
/// Special events: legendary dungeon, world boss, and hellevator.
pub mod event;
/// Fortress buildings, units, and resources.
pub mod fortress;
/// Guild information, chat, raid attacks, and guild portal.
pub mod guild;
/// The idle clicker minigame.
pub mod idle;
/// Items, equipment slots, shops, and active potions.
pub mod items;
/// Pets collection, exploration, and pet battles.
pub mod pets;
/// Time-sensitive rewards, advent calendar, and lucky wheel.
pub mod rewards;
/// Social interactions: lookup, mail, hall of fames, and other entities.
pub mod social;
/// Tavern activities, quests, dice game, toilet, and expeditions.
pub mod tavern;
/// The underworld buildings, units, and souls.
pub mod underworld;
/// Unlockable features like scrapbook, blacksmith, and witch.
pub mod unlockables;

use std::{borrow::Borrow, collections::HashMap};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use dungeons::CompanionClass;
use enum_map::EnumMap;
pub use event::{hellevator, legendary_dungeon, world_boss};
use fortress::{FortressResourceType, FortressUnitType};
use items::{EQUIPMENT_PARSE_LEN, ITEM_PARSE_LEN};
use log::{error, warn};
use strum::{EnumCount, IntoEnumIterator};
use underworld::{TOWER_BLOCK_LEN, TOWER_RESPONSE_LEN, UnderworldBuildingType};
use unlockables::{HabitatType, HellevatorDailyReward};

pub use crate::error::{Result, SFError};
use crate::{
    command::{AttributeType, ExpeditionSetting, Language, ShopType},
    misc::{EnumMapGet, from_sf_string},
    response::{Response, ResponseVal, Vals},
};

/// Represent the full state of the game at some point in time
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameState {
    /// Everything, that can be considered part of the character, or his
    /// immediate surrounding and not the rest of the world
    pub character: character::Character,
    /// Information about quests and work
    pub tavern: tavern::Tavern,
    /// The place to fight other players
    pub arena: arena::Arena,
    /// The last fight, that this player was involved in
    pub last_fight: Option<arena::Fight>,
    /// Both shops. You can access a specific one either with `get()`,
    /// `get_mut()`, or `[]` and the `ShopType` as the key.
    pub shops: EnumMap<ShopType, Shop>,
    /// The average item level offered in the shops
    pub shop_item_lvl: u32,
    /// If the player is in a guild, this will contain information about it
    pub guild: Option<guild::Guild>,
    /// Everything, that is time sensitive, like events, calendar, etc.
    pub specials: rewards::TimedSpecials,
    /// Everything, that can be found under the Dungeon tab
    pub dungeons: dungeons::Dungeons,
    /// Contains information about the underworld, if it has been unlocked
    pub underworld: Option<underworld::Underworld>,
    /// Contains information about the fortress, if it has been unlocked
    pub fortress: Option<fortress::Fortress>,
    /// Information the pet collection, that a player can build over time
    pub pets: Option<unlockables::Pets>,
    /// Contains information about the hellevator, if it is currently active
    pub hellevator: hellevator::HellevatorEvent,
    /// Contains information about the legendary dungeons event, if it is
    /// currently active
    pub legendary_dungeon: legendary_dungeon::LegendaryDungeonEvent,
    /// Contains information about the blacksmith, if it has been unlocked
    pub blacksmith: Option<unlockables::Blacksmith>,
    /// Contains information about the witch, if it has been unlocked
    pub witch: Option<unlockables::Witch>,
    /// Tracker for small challenges, that a player can complete
    pub achievements: unlockables::Achievements,
    /// The boring idle game
    pub idle_game: Option<idle::IdleGame>,
    /// Contains the features this char is able to unlock right now
    pub pending_unlocks: Vec<unlockables::Unlockable>,
    /// Anything related to hall of fames
    pub hall_of_fames: social::HallOfFames,
    /// Contains both other guilds & players, that you can look at via commands
    pub lookup: social::Lookup,
    /// Anything you can find in the mail tab of the official client
    pub mail: social::Mail,
    /// Information about an upcoming, or currently running event. Basically
    /// the tab above Tavern in the official GUI
    pub special_event: Option<event::EventStatus>,
    /// The current state of the world boss. Note that this here does not
    /// remove itself. You should always make sure `special_event` is ongoing
    /// before acting upon any world boss information
    pub world_boss: Option<world_boss::WorldBossEvent>,
    /// The language, that the player has set in the client. This will affect
    /// the language of news messages in the inbox (Mail)
    pub language: Option<Language>,
    /// The raw timestamp, that the server has sent us
    last_request_timestamp: i64,
    /// The amount of sec, that the server is ahead of us in seconds (can be
    /// negative)
    server_time_diff: i64,
}

pub use items::{Shop, ShopPosition};

impl GameState {
    /// Constructs a new `GameState` directly in a `Box` from the provided
    /// response. The response has to be the login response from a `Session`.
    ///
    /// # Errors
    /// If the response contains any errors, or does not contain enough
    /// information about the player to build a full `GameState`, this will
    /// return a `ParsingError`, or `TooShortResponse` depending on the
    /// exact error
    pub fn new_boxed(response: Response) -> Result<Box<Self>, SFError> {
        let mut res = Box::<Self>::default();
        res.update(response)?;
        if res.character.level == 0 || res.character.name.is_empty() {
            return Err(SFError::ParsingError(
                "response did not contain full player state",
                String::new(),
            ));
        }
        Ok(res)
    }

    /// Constructs a new `GameState` from the provided response. The response
    /// has to be the login response from a `Session`.
    ///
    /// # Errors
    /// If the response contains any errors, or does not contain enough
    /// information about the player to build a full `GameState`, this will
    /// return a `ParsingError`, or `TooShortResponse` depending on the
    /// exact error
    pub fn new(response: Response) -> Result<Self, SFError> {
        Self::new_boxed(response).map(|a| *a)
    }

    /// Updates the players information with the new data received from the
    /// server. Any error that is encounters terminates the update process
    ///
    /// # Errors
    /// Mainly returns `ParsingError` if the response does not exactly follow
    /// the expected length, type and layout
    pub fn update<R: Borrow<Response>>(
        &mut self,
        response: R,
    ) -> Result<(), SFError> {
        let response = response.borrow();
        let new_vals = response.values();
        // Because the conversion of all other timestamps relies on the servers
        // timestamp, this has to be set first
        if let Some(ts) = new_vals.get("timestamp").copied() {
            let ts = ts.into("server time stamp")?;
            let server_time = DateTime::from_timestamp(ts, 0).ok_or(
                SFError::ParsingError("server time stamp", ts.to_string()),
            )?;
            self.server_time_diff = (server_time.naive_utc()
                - response.received_at())
            .num_seconds();
            self.last_request_timestamp = ts;
        }
        let server_time = self.server_time();

        self.last_fight = None;
        self.mail.open_claimable = None;

        let mut other_player: Option<social::OtherPlayer> = None;
        let mut other_guild: Option<social::OtherGuild> = None;

        let mut errors = vec![];
        for (key, val) in new_vals.iter().map(|(a, b)| (*a, *b)) {
            let res = self.apply_update_key(
                key,
                val,
                &mut other_player,
                &mut other_guild,
                server_time,
                new_vals,
            );
            if let Err(err) = res {
                errors.push(err);
            }
        }

        if let Some(og) = other_guild {
            self.lookup.guilds.insert(og.name.clone(), og);
        }
        if let Some(other_player) = other_player {
            self.lookup.insert_lookup(other_player);
        }

        // Dungeon portal is unlocked with level 99
        if self.dungeons.portal.is_some() && self.character.level < 99 {
            self.dungeons.portal = None;
        }

        if let Some(pets) = &self.pets
            && pets.rank == 0
        {
            self.pets = None;
        }
        if let Some(t) = &self.guild
            && t.name.is_empty()
        {
            self.guild = None;
        }
        if self.fortress.is_some() && self.character.level < 25 {
            self.fortress = None;
        }
        if let Some(fortress) = &mut self.fortress {
            for (typ, unit) in &mut fortress.units {
                let building_lvl =
                    fortress.buildings.get(typ.training_building()).level;
                let limit_modifier = match typ {
                    FortressUnitType::Magician => 1,
                    FortressUnitType::Archer => 2,
                    FortressUnitType::Soldier => 3,
                };
                unit.limit = building_lvl * limit_modifier;
            }
        }

        if let Some(t) = &self.underworld
            && t.buildings[UnderworldBuildingType::HeartOfDarkness].level < 1
        {
            self.underworld = None;
        }

        // Witch is automatically unlocked with level 66
        if self.witch.is_some() && self.character.level < 66 {
            self.witch = None;
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.remove(0)),
            _ => Err(SFError::NestedError(errors)),
        }
    }

    /// Updates the account-wide resources from a `resources` response:
    /// `[player_id, mushrooms, silver, lucky_coins, quicksand_glasses,
    /// saved_wood, wood_limit, saved_stone, stone_limit, metal, arcane, souls,
    /// fruits...]`.
    pub(crate) fn update_resources(&mut self, data: &Vals<'_>) {
        data.ignore(0, "character id");
        self.character.mushrooms = data.u32_or(1, 0);
        self.character.silver = data.u64_or(2, 0);
        self.specials.wheel.lucky_coins = data.u32_or(3, 0);
        self.tavern.quicksand_glasses = data.u32_or(4, 0);

        let fortress = self.fortress.get_or_insert_default();
        fortress
            .resources
            .get_mut(FortressResourceType::Wood)
            .current = data.u64_or(5, 0);
        data.ignore(6, "fortress max secret wood");
        fortress
            .resources
            .get_mut(FortressResourceType::Stone)
            .current = data.u64_or(7, 0);
        data.ignore(8, "fortress max secret stone");

        let bs = self.blacksmith.get_or_insert_default();
        bs.metal = data.u64_or(9, 0);
        bs.arcane = data.u64_or(10, 0);

        self.underworld.get_or_insert_default().souls_current =
            data.u64_or(11, 0);

        let pets = self.pets.get_or_insert_default();
        for (e_pos, element) in HabitatType::iter().enumerate() {
            pets.habitats.get_mut(element).fruits = data.u16_or(12 + e_pos, 0);
        }

        if data.len() > 17 {
            data.ignore(17..data.len(), "unmodelled trailing resource fields");
        }
    }

    /// Updates language and questing preference settings from a `usersettings`
    /// response.
    pub(crate) fn update_user_settings(&mut self, data: &Vals<'_>) {
        if let Some(lang_str) = data.get_str(0) {
            self.language = Language::parse(lang_str);
        }
        data.ignore(1..4, "unmodelled user settings flags");
        self.tavern.questing_preference = match data.get_str(4) {
            Some("a") => ExpeditionSetting::PreferExpeditions,
            Some("0" | "b") | None => ExpeditionSetting::PreferQuests,
            Some(x) => {
                error!("Weird expedition settings: {x}");
                ExpeditionSetting::PreferQuests
            }
        };
        if data.len() > 5 {
            data.ignore(5..data.len(), "unmodelled trailing user settings");
        }
    }

    /// Updates the character, guild demon portal damage bonus, and dungeon
    /// portal player HP bonus from an `ownplayersavecharacter` response of 70
    /// values.
    pub(crate) fn update_character_save(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.character.update(data, server_time)?;
        self.guild.get_or_insert_default().portal.damage_bonus =
            data.u8_or(26, 0);
        self.dungeons.portal.get_or_insert_default().player_hp_bonus =
            data.u16_or(28, 0);
        Ok(())
    }

    /// Updates the companion and underworld state from an `owntower`
    /// (`towerSave`) response: three companion blocks then the underworld.
    pub(crate) fn update_tower(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        // The companion level starts each block, the three values before the
        // first one are the header
        const BLOCK_OFFSET: usize = 3;
        // The own player id, a 0 and a constant 100
        data.ignore(0, "own player id");
        data.ignore(1, "always 0");
        data.ignore(2, "always 100");

        let companions = self.dungeons.companions.get_or_insert_default();
        for (i, class) in CompanionClass::iter().enumerate() {
            // A block is TOWER_BLOCK_LEN values: the companion, its underworld
            // unit and the trailing value after it
            let start = BLOCK_OFFSET + i * TOWER_BLOCK_LEN;
            companions[class]
                .update(&data.window(start..start + TOWER_BLOCK_LEN))?;
        }

        // Why would they include this in the tower response???
        let underworld = self.underworld.get_or_insert_default();
        for (i, unit) in underworld.units.values_mut().enumerate() {
            // The unit is the four values before its block's trailing value
            let start = BLOCK_OFFSET + (i + 1) * TOWER_BLOCK_LEN - 5;
            unit.update(&data.window(start..start + 4))?;
        }
        // The rest of the underworld state follows the last block
        let tail = BLOCK_OFFSET + CompanionClass::COUNT * TOWER_BLOCK_LEN;
        underworld
            .update(&data.window(tail..TOWER_RESPONSE_LEN), server_time)?;
        Ok(())
    }

    /// Updates the character status, current action, tavern and timer details
    /// from a `characterstatus` response of 33 values.
    pub(crate) fn update_character_status(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0, "status version? seen 1 or 2");

        self.tavern.current_action =
            tavern::CurrentAction::parse(&data.window(1..4), server_time)?;

        data.ignore(4, "action start timestamp");
        self.tavern.beer_max = data.u8_or(5, 0);
        self.tavern.thirst_for_adventure_sec = data.u32_or(6, 0);
        self.tavern.beer_drunk = data.u8_or(7, 0);

        self.specials.calendar.collected = data.usize_or(8, 245);
        self.specials.calendar.next_possible =
            data.server_time(9, server_time)?;

        data.ignore(10..20, "unmodelled characterstatus fields");

        self.pets.get_or_insert_default().next_free_exploration =
            data.server_time(20, server_time)?;
        self.dungeons.next_free_fight = data.server_time(21, server_time)?;
        if let Some(start) = data.server_time(22, server_time)? {
            self.legendary_dungeon
                .active
                .get_or_insert_default()
                .healing_start = Some(start);
        }

        data.ignore(23..33, "unmodelled characterstatus trailing fields");
        Ok(())
    }

    /// Returns the time of the server. This is just an 8 byte copy behind the
    /// scenes, so feel free to NOT cache/optimize calling this in any way
    #[must_use]
    pub fn server_time(&self) -> ServerTime {
        ServerTime(self.server_time_diff)
    }

    /// Given a header value like "fight4", this would give you the
    /// corresponding fight[3]. In case that does not exist, it will be created
    /// w/ the default
    #[must_use]
    fn get_fight(&mut self, header_name: &str) -> &mut arena::SingleFight {
        let id = fight_no_from_header(header_name);
        let fights = &mut self.last_fight.get_or_insert_default().fights;

        if fights.len() < id {
            fights.resize(id, arena::SingleFight::default());
        }
        #[allow(clippy::unwrap_used)]
        fights.get_mut(id - 1).unwrap()
    }

    /// Updates the gamestate with the given key and value
    #[allow(clippy::match_same_arms)]
    fn apply_update_key(
        &mut self,
        key: &str,
        val: ResponseVal<'_>,
        other_player: &mut Option<social::OtherPlayer>,
        other_guild: &mut Option<social::OtherGuild>,
        server_time: ServerTime,
        all_values: &HashMap<&str, ResponseVal<'_>>,
    ) -> Result<(), SFError> {
        match key {
            "timestamp" => {
                // Handled above
            }
            "Success" | "sucess" => {
                // Whatever we did worked. Note that the server also
                // sends this for bad requests from time to time :)
            }
            "login count" | "sessionid" | "cryptokey" | "cryptoid" => {
                // Should already be handled when receiving the response
            }
            "preregister"
            | "languagecodelist"
            | "tracking"
            | "skipvideo"
            | "webshopid"
            | "cidstring"
            | "mountexpired"
            | "tracking_netto"
            | "tracking_coins"
            | "tutorial_game_entry" => {
                // Stuff that looks irrellevant
            }
            "ownplayername" => {
                self.character.name.set(val.as_str());
            }
            "owndescription" => {
                self.character.description = from_sf_string(val.as_str());
            }
            "wagesperhour" => {
                self.tavern.guard_wage = val.into("tavern wage")?;
            }
            "skipallow" => {
                let raw_skip = val.into::<i32>("skip allow")?;
                self.tavern.mushroom_skip_allowed = raw_skip != 0;
            }
            "cryptoid not found" => return Err(SFError::ConnectionError),
            "ownplayersave" => {
                // Goodbye old friend...
            }
            "owngroupname" => {
                self.guild.get_or_insert_default().name.set(val.as_str());
            }
            "sfhomeid" => {}
            "backpack" => {
                self.character.inventory.update(&val.list(), server_time)?;
            }
            "itemlevelshop" => {
                self.shop_item_lvl = val.into("shop lvl")?;
            }
            "storeitemsshakes" => {
                let shop = self.shops.get_mut(ShopType::Weapon);
                shop.typ = ShopType::Weapon;
                shop.update(&val.values::<114>()?, server_time)?;
            }
            "questofferitems" => {
                self.tavern.update_quest_items(&val.list(), server_time);
            }
            "toiletstate" => {
                self.tavern
                    .toilet
                    .get_or_insert_default()
                    .update_sacrifices(&val.values::<3>()?);
            }
            "companionequipment" => {
                self.dungeons
                    .update_companion_equipment(&val.list(), server_time)?;
            }
            "storeitemsfidget" => {
                let shop = self.shops.get_mut(ShopType::Magic);
                shop.typ = ShopType::Magic;
                shop.update(&val.values::<114>()?, server_time)?;
            }
            "ownplayersaveequipment" => {
                self.character.equipment.update(
                    &val.values::<EQUIPMENT_PARSE_LEN>()?,
                    server_time,
                )?;
            }
            "systemmessagelist" => {}
            "newslist" => {
                self.mail.update_news_inbox(&val.list_by(';'), server_time);
            }
            "dummieequipment" => {
                let data = val.values::<EQUIPMENT_PARSE_LEN>()?;
                self.character.mannequin =
                    Some(items::Equipment::parse(&data, server_time)?);
            }
            "owntower" => self.update_tower(
                &val.values::<TOWER_RESPONSE_LEN>()?,
                server_time,
            )?,
            "owngrouprank" => {
                self.guild.get_or_insert_default().rank =
                    val.into("group rank")?;
            }
            "owngroupattack" | "owngroupdefense" => {
                // Annoying
            }
            "owngrouprequirement" | "othergrouprequirement" => {
                // TODO:
            }
            "owngroupsave" => {
                self.guild
                    .get_or_insert_default()
                    .update_group_save(&val.list(), server_time)?;
            }
            "owngroupmember" => self
                .guild
                .get_or_insert_default()
                .update_member_names(&val.list_by(',')),
            "owngrouppotion" => {
                self.guild
                    .get_or_insert_default()
                    .update_member_potions(&val.list_by(','));
            }
            "unitprice" => {
                self.fortress
                    .get_or_insert_default()
                    .update_unit_prices(&val.values::<12>()?)?;
            }
            "dicestatus" => {
                self.tavern.dice_game.update_status(&val.list());
            }
            "dicereward" => {
                self.tavern.dice_game.update_reward(&val.values::<2>()?)?;
            }
            "chathistory" => {
                self.guild.get_or_insert_default().chat =
                    guild::ChatMessage::parse_messages(val.as_str());
            }
            "chatwhisper" => {
                self.guild.get_or_insert_default().whispers =
                    guild::ChatMessage::parse_messages(val.as_str());
            }
            "upgradeprice" => {
                self.fortress
                    .get_or_insert_default()
                    .update_unit_upgrade_info(&val.values::<9>()?)?;
            }
            "unitlevel" => {
                self.fortress
                    .get_or_insert_default()
                    .update_levels(&val.values::<4>()?)?;
            }
            "fortressprice" => {
                self.fortress
                    .get_or_insert_default()
                    .update_prices(&val.values::<52>()?)?;
            }
            "Arenarank" => {
                if let Some(uw) = self.underworld.as_mut() {
                    uw.lure_suggestion = val
                        .as_str()
                        .parse::<u32>()
                        .ok()
                        .map(underworld::LureSuggestion);
                }
            }
            "witch" => {
                // old witch data without price
            }
            "witchshop" => {
                self.witch.get_or_insert_default().update(&val.list());
            }
            "underworldupgradeprice" => {
                self.underworld
                    .get_or_insert_default()
                    .update_underworld_unit_prices(&val.values::<9>()?)?;
            }
            "unlockfeature" => {
                self.pending_unlocks =
                    unlockables::Unlockable::parse(&val.list())?;
            }
            "dungeonprogresslight" => self.dungeons.update_progress(
                &val.values::<37>()?,
                dungeons::DungeonType::Light,
            )?,
            "dungeonprogressshadow" => self.dungeons.update_progress(
                &val.values::<37>()?,
                dungeons::DungeonType::Shadow,
            )?,
            "portalprogress" => {
                self.dungeons
                    .portal
                    .get_or_insert_default()
                    .update(&val.values::<3>()?, server_time)?;
            }
            "owntowerlevel" => {
                // Already in dungeons
            }
            "serverversion" => {
                // Handled in session
            }
            "stoneperhournextlevel" => {
                self.fortress
                    .get_or_insert_default()
                    .resources
                    .get_mut(FortressResourceType::Stone)
                    .production
                    .per_hour_next_lvl = val.into("stone next lvl")?;
            }
            "woodperhournextlevel" => {
                self.fortress
                    .get_or_insert_default()
                    .resources
                    .get_mut(FortressResourceType::Wood)
                    .production
                    .per_hour_next_lvl = val.into("wood next lvl")?;
            }
            "shadowlevel" | "dungeonlevel" => {
                // We just look at the db
            }
            "gttime" => {
                self.hellevator.update_time(&val.list(), server_time);
            }
            "gtsave" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update(&val.values::<32>()?, server_time)?;
            }
            "eventstatus" => {
                self.special_event = val
                    .values::<5>()
                    .and_then(|data| {
                        event::EventStatus::parse(&data, server_time)
                    })
                    .inspect_err(|a| {
                        log::warn!("Could not parse event status: {a}");
                    })
                    .ok();
            }
            "wbcharacter" => {
                self.world_boss
                    .get_or_insert_default()
                    .update(&val.values::<15>()?, server_time)?;
            }
            "wbtower" => {
                self.world_boss.get_or_insert_default().battle = Some(
                    world_boss::WorldBossBattle::parse(&val.values::<106>()?),
                );
            }
            "wbupgradestore" => {
                self.world_boss
                    .get_or_insert_default()
                    .update_upgrade_store(&val.list());
            }
            "wbammostore" => {
                self.world_boss
                    .get_or_insert_default()
                    .update_ammo_store(&val.list());
            }
            "wbammo" => {
                // Special ammo
                let ammo =
                    world_boss::WorldBossProjectile::parse(&val.values::<3>()?);
                self.world_boss.get_or_insert_default().projectile = ammo;
            }
            "wbupgrade" => {
                self.world_boss.get_or_insert_default().catapult =
                    world_boss::WorldBossCatapult::parse(
                        &val.values::<17>()?,
                        server_time,
                    )?;
            }
            "wbrankinginternationalmax" => {
                self.world_boss
                    .get_or_insert_default()
                    .max_international_ranks = val.into("wb int rank max").ok();
            }
            "wbrankingworldmax" => {
                self.world_boss.get_or_insert_default().max_world_ranks =
                    val.into("wb int rank world max").ok();
            }
            "wbrankingfightermax" => {
                self.world_boss.get_or_insert_default().max_fighter_ranks =
                    val.into("wb int rank fighter max").ok();
            }
            "wbdailychests" => {
                self.world_boss
                    .get_or_insert_default()
                    .update_daily_chests(&val.values::<5>()?);
            }
            "maxrank" => {
                self.hall_of_fames.players_total = val.into("player count")?;
            }
            "achievement" => {
                self.achievements.update(&val.list());
            }
            "groupskillprice" => {
                self.guild
                    .get_or_insert_default()
                    .update_group_prices(&val.values::<6>()?);
            }
            "soldieradvice" => {
                let advice: u16 = val.into("soldier advice")?;
                if advice > 0 {
                    other_player
                        .get_or_insert_default()
                        .fortress
                        .get_or_insert_default()
                        .soldier_advice = advice;
                }
            }
            "owngroupdescription" => self
                .guild
                .get_or_insert_default()
                .update_description_embed(val.as_str()),
            "idle" => {
                self.idle_game =
                    idle::IdleGame::parse(&val.list(), server_time);
            }
            "resources" => {
                self.update_resources(&val.list());
            }
            "chattime" => {
                // let _chat_time = server_time
                //     .convert_to_local(val.into("chat time")?, "chat
                // time"); Pretty sure this is the time something last
                // happened in chat, but nobody cares and messages have a
                // time
            }
            "maxpetlevel" => {
                self.pets.get_or_insert_default().max_pet_level =
                    val.into("max pet lvl")?;
            }
            "otherdescription" => {
                other_player.get_or_insert_default().description =
                    from_sf_string(val.as_str());
            }
            "otherplayergroupname" => {
                let guild =
                    Some(val.as_str().to_string()).filter(|a| !a.is_empty());
                other_player.get_or_insert_default().guild = guild;
            }
            "otherplayername" => {
                other_player.get_or_insert_default().name.set(val.as_str());
            }
            "otherplayersaveequipment" => {
                other_player.get_or_insert_default().equipment.update(
                    &val.values::<EQUIPMENT_PARSE_LEN>()?,
                    server_time,
                )?;
            }
            "fortresspricereroll" => {
                self.fortress.get_or_insert_default().opponent_reroll_price =
                    val.into("fortress reroll")?;
            }
            "fortresswalllevel" => {
                self.fortress.get_or_insert_default().wall_combat_lvl =
                    val.into("fortress wall lvl")?;
            }
            "dragongoldbonus" => {
                self.character.mount_dragon_refund = val.into("dragon gold")?;
            }
            "wheelresult" => {
                // NOTE: These are the reqs to unlock the upgrade, not a
                // check if it is actually upgraded
                let upgraded = self.character.level >= 95
                    && self.pets.is_some()
                    && self.underworld.is_some();
                self.specials
                    .wheel
                    .update_result(&val.values::<2>()?, upgraded)?;
            }
            "dailyreward" => {
                // Dead since last update
            }
            "calenderreward" => {
                // Probably removed and should be irrelevant
            }
            "oktoberfest" => {
                // Not sure if this is still used, but it seems to just be
                // empty.
                if !val.as_str().is_empty() {
                    warn!("oktoberfest response is not empty: {val}");
                }
            }
            "usersettings" => {
                self.update_user_settings(&val.list());
            }
            "mailinvoice" => {
                // Incomplete email address
            }
            "calenderinfo" => {
                // This is twice in the original response.
                // This API sucks LMAO
                self.specials.calendar.update_rewards(&val.list())?;
            }
            "othergroupattack" => {
                other_guild.get_or_insert_default().attacks =
                    Some(val.to_string());
            }
            "othergroupdefense" => {
                other_guild.get_or_insert_default().defends_against =
                    Some(val.to_string());
            }
            "inboxcapacity" => {
                self.mail.inbox_capacity = val.into("inbox cap")?;
            }
            "magicregistration" => {
                // Pretty sure this means you have not provided a pw or
                // mail. Just a name and clicked play
            }
            "Ranklistplayer" => {
                self.hall_of_fames.update_players(&val.list_by(';'));
            }
            "ranklistgroup" => {
                self.hall_of_fames.update_guilds(&val.list_by(';'));
            }
            "maxrankgroup" => {
                self.hall_of_fames.guilds_total = Some(val.into("guild max")?);
            }
            "maxrankPets" => {
                self.hall_of_fames.pets_total = Some(val.into("pet rank max")?);
            }
            "RanklistPets" => {
                self.hall_of_fames.update_pets(&val.list_by(';'));
            }
            "ranklistfortress" | "Ranklistfortress" => {
                self.hall_of_fames.update_fortresses(&val.list_by(';'));
            }
            "ranklistunderworld" => {
                self.hall_of_fames.update_underworlds(&val.list_by(';'));
            }
            "gamblegoldvalue" => {
                self.tavern.gamble_result =
                    Some(tavern::GambleResult::SilverChange(
                        val.into("gold gamble")?,
                    ));
            }
            "gamblecoinvalue" => {
                self.tavern.gamble_result =
                    Some(tavern::GambleResult::MushroomChange(
                        val.into("gold gamble")?,
                    ));
            }
            "maxrankFortress" => {
                self.hall_of_fames.fortresses_total =
                    Some(val.into("fortress max")?);
            }
            "underworldprice" => self
                .underworld
                .get_or_insert_default()
                .update_building_prices(&val.values::<30>()?)?,
            "owngroupknights" => self
                .guild
                .get_or_insert_default()
                .update_group_knights(&val.list_by(',')),
            "friendlist" => {
                self.character.update_relations(&val.list_by(';'));
            }
            "legendaries" => {
                if val.as_str().chars().any(|a| a != 'A') {
                    warn!("Found a legendaries value, that is not just AAA..");
                }
            }
            "smith" => {
                self.blacksmith
                    .get_or_insert_default()
                    .update(&val.list(), server_time);
            }
            "fortressGroupPrice" => {
                self.fortress
                    .get_or_insert_default()
                    .hall_of_knights_upgrade_price =
                    fortress::FortressCost::parse(&val.values::<4>()?)?;
            }
            "goldperhournextlevel" => {
                // I dont think this matters
            }
            "underworldmaxsouls" => {
                // This should already be in resources
            }
            "dailytaskrewardpreview" => {
                self.specials.tasks.daily.update_rewards(&val.list())?;
            }
            "expeditionevent" => {
                self.tavern
                    .expeditions
                    .update_event(&val.list(), server_time);
            }
            "expeditions" => {
                self.tavern.expeditions.update_available(&val.list());
            }
            "expeditionrewardresources" => {
                // I would assume, that everything we get is just update
                // elsewhere, so I dont care about parsing this
            }
            "expeditionreward" => {
                // This works, but I dont think anyone cares about that. It
                // will just be in the inv. anyways
                // let data:Vec<i64> = val.into_list("expedition reward")?;
                // for chunk in data.chunks_exact(ITEM_PARSE_LEN){
                //     let item = Item::parse(chunk, server_time);
                //     println!("{item:#?}");
                // }
            }
            "expeditionmonster" => {
                self.tavern.expeditions.active.get_or_insert_default().boss =
                    tavern::ExpeditionBoss::parse(&val.values::<2>()?);
            }
            "expeditionhalftime" => {
                self.tavern
                    .expeditions
                    .active
                    .get_or_insert_default()
                    .update_halftime(&val.list());
            }
            "expeditionstate" => {
                self.tavern
                    .expeditions
                    .active
                    .get_or_insert_default()
                    .update_state(&val.list(), server_time)?;
            }
            "expeditioncrossroad" => {
                self.tavern
                    .expeditions
                    .active
                    .get_or_insert_default()
                    .update_encounters(&val.list());
            }
            "eventtasklist" => {
                self.specials.tasks.event.update_tasks(&val.list())?;
            }
            "eventtaskrewardpreview" => {
                self.specials.tasks.event.update_rewards(&val.list())?;
            }
            "dailytasklist" => {
                self.specials.tasks.daily.update_tasks(&val.list())?;
            }
            "eventtaskinfo" => {
                self.specials
                    .tasks
                    .event
                    .update_info(&val.values::<3>()?, server_time)?;
            }
            "scrapbook" => {
                self.character.scrapbook =
                    unlockables::ScrapBook::parse(&val.list());
            }
            "dungeonfaces" | "shadowfaces" => {
                // Gets returned after winning a dungeon fight. This looks a
                // bit like a reward, but that should be handled in fight
                // parsing already?
            }
            "messagelist" => {
                self.mail.update_inbox(&val.list_by(';'), server_time);
            }
            "messagetext" => {
                self.mail.open_msg = Some(from_sf_string(val.as_str()));
            }
            "combatloglist" => {
                self.mail.update_combat_log(&val.list_by(';'), server_time);
            }
            "maxupgradelevel" => {
                self.fortress.get_or_insert_default().building_max_lvl =
                    val.into("max upgrade lvl")?;
            }
            "singleportalenemylevel" => {
                self.dungeons.portal.get_or_insert_default().enemy_level =
                    val.into("portal lvl").unwrap_or(u32::MAX);
            }
            "ownpetsstats" => {
                self.pets
                    .get_or_insert_default()
                    .update_pet_stat(&val.values::<17>()?);
            }
            "ownpets" => {
                self.pets
                    .get_or_insert_default()
                    .update(&val.list(), server_time)?;
            }
            "petsdefensetype" => {
                let pet_id = val.into("pet def typ")?;
                self.pets.get_or_insert_default().opponent.habitat =
                    Some(HabitatType::from_typ_id(pet_id).ok_or(
                        SFError::ParsingError(
                            "pet def typ",
                            format!("{pet_id}"),
                        ),
                    )?);
            }
            "otherplayersavecharacter" => {
                other_player
                    .get_or_insert_default()
                    .update(&val.list(), server_time)?;
            }
            "otherplayersavepotions" => {
                other_player.get_or_insert_default().active_potions =
                    items::parse_active_potions(
                        &val.values::<10>()?,
                        server_time,
                    )?;
            }
            "otherplayer" => {
                let data = val.list();
                #[allow(deprecated)]
                {
                    other_player.get_or_insert_default().guild_joined =
                        data.server_time(166, server_time)?;
                }
            }
            "otherplayerfriendstatus" => {
                other_player.get_or_insert_default().relationship = val
                    .into::<i32>("other friend")
                    .ok()
                    .and_then(num_traits::FromPrimitive::from_i32)
                    .unwrap_or_default();
            }
            "otherplayerpetbonus" => {
                other_player
                    .get_or_insert_default()
                    .update_pet_bonus(&val.values::<6>()?)?;
            }
            "otherplayerunitlevel" => {
                other_player
                    .get_or_insert_default()
                    .update_unit_levels(&val.list());
            }
            "petsrank" => {
                self.pets.get_or_insert_default().rank =
                    val.into("pet rank")?;
            }

            "maxrankUnderworld" => {
                self.hall_of_fames.underworlds_total =
                    Some(val.into("mrank under")?);
            }
            "otherplayerfortressrank" => {
                let rank = val.into::<i64>("other player fortress rank")?;
                other_player
                    .get_or_insert_default()
                    .update_fortress_rank(rank);
            }
            "workreward" => {
                // Should be irrelevant
            }
            x if x.starts_with("winnerid") => {
                // For all winnerid's, except the last one, the winnerid
                // value contains the fightversion as well
                let raw_winner_id = val
                    .as_str()
                    .split_once(|a: char| !a.is_ascii_digit())
                    .map_or(val.as_str(), |a| a.0);
                if let Ok(winner_id) = raw_winner_id.parse() {
                    self.get_fight(x).winner_id = winner_id;
                } else {
                    error!("Invalid winner id: {raw_winner_id}");
                }
            }
            "fightresult" => {
                let data = val.list();
                self.last_fight.get_or_insert_default().update_result(&data);
            }
            "battlerewarditem" => {
                // The item won in the fight, encoded like a regular item
                let data = val.values::<ITEM_PARSE_LEN>()?;
                let item = items::Item::parse(&data, server_time)?;
                self.last_fight.get_or_insert_default().item_won = item;
            }
            x if x.starts_with("fightheader") => match val.values::<99>() {
                Ok(data) => self.get_fight(x).update_fighters(&data),
                Err(err) => {
                    warn!("Failed to parse the fight header: {err}");
                }
            },
            "fightgroups" => match val.values_by::<4>(',') {
                Ok(data) => {
                    self.last_fight
                        .get_or_insert_default()
                        .update_groups(&data);
                }
                Err(err) => {
                    warn!("Failed to parse the fight groups: {err}");
                }
            },
            "fightadditionalplayers" => {
                // This should be players in guild battles, that have not
                // participapted. I dont think this matters
            }
            "fightversion" => {
                // This key is unreliable and partially merged into
                // winnerid, so I just parse this in the fight response
                // below, where it is actually used
            }
            x if x.starts_with("fight") && x.len() <= 7 => {
                let fight_version: u32 = all_values
                    .get("fightversion")
                    .and_then(|v| v.as_str().parse().ok())
                    .unwrap_or(1);
                let data = val.list();
                self.get_fight(x).update_rounds(&data, fight_version);
            }
            "othergroupname" => {
                other_guild.get_or_insert_default().name.set(val.as_str());
            }
            "othergrouprank" => {
                other_guild.get_or_insert_default().rank =
                    val.into("other group rank")?;
            }
            "othergroupfightcost" => {
                other_guild.get_or_insert_default().attack_cost =
                    val.into("other group fighting cost")?;
            }
            "othergroupmember" => {
                other_guild
                    .get_or_insert_default()
                    .update_member_names(&val.list_by(','));
            }
            "othergroupdescription" => {
                other_guild
                    .get_or_insert_default()
                    .update_description_embed(&val.list_by('§'));
            }
            "othergroup" => {
                other_guild
                    .get_or_insert_default()
                    .update(&val.list(), server_time)?;
            }
            "reward" => {
                // This is the task reward, which you should already know
                // from collecting
            }
            "gtdailypoints" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .guild_points_today = val.into("gtdaily").unwrap_or(0);
            }
            "gtchest" => {
                // 2500/0/5000/1/7500/2/10000/0/12500/1/15000/2/17500/0/
                // 20000/1/22500/2/25000/0/27500/1/30000/2/32500/0/35000/1/
                // 37500/2/40000/0/42500/1/45000/2/47500/0/50000/1/57500/2/
                // 65000/0/72500/1/80000/2/87500/0/95000/1/102500/2/110000/
                // 0/117500/1/125000/2/137500/0/150000/1/162500/2/175000/0/
                // 187500/1/200000/2/212500/0/225000/1/237500/2/250000/0/
                // 272500/1/295000/2/317500/0/340000/1/362500/2/385000/0/
                // 407500/1/430000/2/452500/0/475000/1
            }
            "gtraidparticipants" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update_raid_participants(&val.list(), |f| {
                        &mut f.today_assigned
                    });
            }
            "gtraidparticipantsyesterday" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update_raid_participants(&val.list(), |f| {
                        &mut f.yesterday_assigned
                    });
            }
            "gtrank" => {
                self.hellevator.active.get_or_insert_default().guild_rank =
                    val.into("gt rank").unwrap_or(0);
            }
            "gtrankingmax" => {
                self.hall_of_fames.hellevator_total =
                    val.into("gt rank max").ok();
            }
            "gtbracketlist" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update_brackets(&val.list());
            }
            "gtraidfights" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update_raid_fights(&val.list(), server_time);
            }
            "gtmonsterreward" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update_monster_rewards(&val.list());
            }
            "gtdailyreward" => {
                self.hellevator.active.get_or_insert_default().rewards_today =
                    HellevatorDailyReward::parse(&val.list());
            }
            "gtdailyrewardnext" => {
                self.hellevator.active.get_or_insert_default().rewards_next =
                    HellevatorDailyReward::parse(&val.list());
            }
            "gtdailyrewardyesterday" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .rewards_yesterday =
                    HellevatorDailyReward::parse(&val.list());
            }
            "gtdailyrewardclaimed" => {
                if let Some(hellevator) = self.hellevator.active.as_mut() {
                    // This response key is sent when either yesterday's or
                    // today's daily reward was claimed. To check whether
                    // yesterday's daily reward was claimed, we check if
                    // "gtdailyreward" is missing in the response, since it
                    // is only included if today's daily reward was claimed.
                    if !all_values.contains_key("gtdailyreward") {
                        // The game doesn't update this value itself, so we
                        // do it manually.
                        hellevator.rewards_yesterday = None;
                    }
                }
            }
            "gtranking" => {
                self.hall_of_fames.update_hellevator(&val.list_by(';'));
            }
            "gtpreviewreward" => {
                // TODO: these are the previews of the rewards per rank
                // 1:17/0/1/16/0/1/8/1/64200/9/1/96300/4/1/3201877800/,2:18/
                // 0/1/16/0/1/8/1/64200/9/1/96300/4/1/3201877800/,3:19/0/1/
                // 16/0/1/8/1/64200/9/1/96300/4/1/3201877800/,4:16/0/1/8/1/
                // 61632/9/1/92448/4/1/3041783910/,5:16/0/1/8/1/59064/9/1/
                // 88596/4/1/2881690020/,6:16/0/1/8/1/56496/9/1/84744/4/1/
                // 2721596130/,7:16/0/1/8/1/53928/9/1/80892/4/1/2561502240/,
                // 8:16/0/1/8/1/51360/9/1/77040/4/1/2401408350/,9:16/0/1/8/
                // 1/48792/9/1/73188/4/1/2241314460/,10:16/0/1/8/1/46224/9/
                // 1/69336/4/1/2241314460/,11:16/0/1/8/1/43656/9/1/65484/4/
                // 1/2081220570/,12:16/0/1/8/1/41088/9/1/61632/4/1/
                // 2081220570/,13:16/0/1/8/1/38520/9/1/57780/4/1/1921126680/
                // ,14:16/0/1/8/1/35952/9/1/53928/4/1/1921126680/,15:16/0/1/
                // 8/1/33384/9/1/50076/4/1/1761032790/,16:16/0/1/8/1/30816/
                // 9/1/46224/4/1/1761032790/,17:8/1/28248/9/1/42372/4/1/
                // 1600938900/,18:8/1/25680/9/1/38520/4/1/1600938900/,19:4/
                // 1/1440845010/,20:4/1/1280751120/,21:4/1/1120657230/,22:4/
                // 1/960563340/,23:4/1/800469450/,24:4/1/640375560/,25:4/1/
                // 480281670/,
            }
            "gtmonster" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .current_monster =
                    unlockables::HellevatorMonster::parse(&val.list()).ok();
            }
            "gtbonus" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .daily_treat_bonus =
                    unlockables::HellevatorTreatBonus::parse(&val.list()).ok();
            }
            "pendingrewards" => {
                self.mail.update_claimables(&val.list(), server_time);
            }
            "pendingrewardressources" => {
                self.mail
                    .open_claimable
                    .get_or_insert_default()
                    .update_resources(&val.list());
            }
            "pendingreward" => {
                self.mail
                    .open_claimable
                    .get_or_insert_default()
                    .update_items(&val.list(), server_time);
            }
            "fightablegroups" => {
                self.guild
                    .get_or_insert_default()
                    .update_fightable_targets(&val.list())?;
            }
            "adventscalendar" => {
                self.specials.update_advent_calendar(&val.list())?;
            }
            "fortresschances" => {
                // chances for different gems to drop in the gem mine / 100
                // big/medium/small/orange/black/others
                // 3334/3333/3333/0/1700/8300
            }
            "deedsandtitlesplayersave" => {
                // The deeds of glory of the player
                // {pid}/{glory}/{rank}/{bronze}/{silver}/{gold}/2/2/0/1/58/0/
                // 59/0/60/0/0/0/0/0/0/0
            }
            "deedshelves" => {
                // deedshelves (subkey => 1)
                // 1
            }
            "titleinfos" => {
                // let data:Vec<_> = val.as_str().split('/').collect();
                // 1    // active title type?
                // 2    // hero titles?
                // 1644 // max rank?

                // Top 3?
                // for slice in data.skip(3, "titleinfos")?.chunks_exact(2) {
                // LordBurggraf/1761696040
                // scyth3/1761696052/
                // Deinnachbar/1761696060

                // }
            }
            "titleplayerinfos" => {

                // {pid}/2/2/0/{rank}/1761884016/482/10/0/217/1761821390/482
            }
            "fortressstorage" => {
                self.fortress
                    .get_or_insert_default()
                    .update_resources(&val.values::<16>()?, server_time)?;
            }
            "fortressunits" => {
                self.fortress
                    .get_or_insert_default()
                    .update_units(&val.values::<15>()?, server_time)?;
            }
            "fortress" => {
                self.fortress
                    .get_or_insert_default()
                    .update(&val.values::<27>()?, server_time)?;
            }
            "wheel" => {
                self.specials
                    .wheel
                    .update(&val.values::<3>()?, server_time)?;
            }
            "dice" => {
                self.tavern
                    .dice_game
                    .update(&val.values::<2>()?, server_time);
            }
            "charactergroup" => {
                self.guild
                    .get_or_insert_default()
                    .update_character_group(&val.list(), server_time);
            }
            "arena" => self.arena.update(&val.values::<7>()?, server_time)?,
            "ownplayersavepotions" => {
                self.character.active_potions = items::parse_active_potions(
                    &val.values::<10>()?,
                    server_time,
                )?;
            }
            "arcanetoilet" => {
                let data = val.values::<4>()?;
                let toilet_lvl = data.u32_or(0, 0);
                if toilet_lvl > 0 {
                    self.tavern.toilet.get_or_insert_default().update(&data)?;
                } else {
                    data.ignore_all("toilet not yet unlocked");
                }
            }
            "vipstatus" => {
                other_player.get_or_insert_default().is_vip =
                    val.as_str() != "0";
            }
            "characterstatus" => {
                self.update_character_status(
                    &val.values::<33>()?,
                    server_time,
                )?;
            }
            "ownplayersavecharacter" => {
                self.update_character_save(&val.values::<70>()?, server_time)?;
            }
            "adventure" => {
                self.tavern.update_quests(&val.values::<23>()?)?;
            }
            "events" => {
                self.specials.events.update(&val.list(), server_time)?;
            }
            "tavernspecialend" | "tavernspecialsub" | "tavernspecial" => {
                // Removed old way to serve events
            }
            "subscriptionstatus" => {}
            // Legendary Dungeons
            "iadungeonchances" => {
                // IDK
            }
            "iadungeontime" => {
                self.legendary_dungeon
                    .update_time(&val.values::<4>()?, server_time)?;
            }
            "iadungeonstatstotal" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.total_stats =
                    legendary_dungeon::TotalStats::parse(&val.list())?;
            }
            "iadungeonstats" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.stats =
                    legendary_dungeon::Stats::parse(&val.values::<5>()?)?;
            }
            "iadungeon" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.update(&val.list())?;
                if !all_values.contains_key("iapendingitems") {
                    dungeons.pending_items.clear();
                }
            }
            "iapendingitems" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_pending_items(&val.list(), server_time)?;
            }
            "ialootitem" => {
                // The stuff, that was looted after a fight
            }
            "iamerchant" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_merchant(&val.list())?;
            }
            "iadungeon20cost" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .heal_quarter_cost = val.into("iadungeon20cost")?;
            }
            "iadungeonsoulstones" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_soul_stones(&val.list())?;
            }
            "iamap" => {
                // 25/-5159/1/-315/
                // 25/-5160/1/-315/
                // 25/-5172/1/-315/
                // 25/-5168/1/-315

                // Parsing this sucks. The first value is amount of levels
                // and the 2. is monster_id, so far so good. The next 2
                // values are weird though. If you change -315 to 315, it
                // counts as having visited the shop one, but I have no
                // idea how to further improve that. In addition, you can
                // inc. 1 in steps of 4 to inc. max shops by one... but
                // that breaks the total count of floors?? It is also
                // unclear how effect & name can be changed, might be
                // dependant on `LegendaryDungeonsEventTheme`? Whatever is
                // may be, I don't think we really need this, as long as
                // it does not contain the gems
            }
            "otherplayerfortressinfo" => {
                other_player
                    .get_or_insert_default()
                    .update_fortress(&val.values::<6>()?)?;
            }
            x if x.contains("average") && x.ends_with("level") => {
                // We do not care about avg. item lvl
            }
            // This is the extra bonus effect all treats get that day
            x if x.contains("dungeonenemies") => {
                // I `think` we do not need this
            }
            x if x.starts_with("attbonus") => {
                // This is always 0s, so I have no idea what this could be
            }
            x if x.starts_with("fightequipment") => {
                // Equipment data for each fighter in multi-fight responses.
                // Format: item_count / 19-value items (different encoding
                // from regular Item — first value is model_id, not type).
                // Always 20 values, one item, in the captured responses
            }
            x if x.starts_with("externaltoolequipment") => {
                // External tool/mount equipment data. 8 values, the first
                // ones look like min/max damage pairs, e.g.
                // 647/865/0/0/1206/1260/0/0
            }
            x if x.starts_with("fightdecoration") => {
                // Cosmetic decoration data: always `0/0/0/0` so far
            }
            x => {
                warn!("Update ignored {x} -> {val:?}");
            }
        }
        Ok(())
    }
}

/// Gets the number if the fight header, so for `fight4` it would return 4 and
/// for fight it would return 1
fn fight_no_from_header(header_name: &str) -> usize {
    let number_str =
        header_name.trim_start_matches(|a: char| !a.is_ascii_digit());
    let id: usize = number_str.parse().unwrap_or(1);
    id.max(1)
}

/// Stores the time difference between the server and the client to parse the
/// response timestamps and to always be able to know the servers (timezoned)
/// time without sending new requests to ask it
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ServerTime(i64);

impl ServerTime {
    /// Converts the raw timestamp from the server to local time with caller
    /// tracking.
    #[must_use]
    #[track_caller]
    pub(crate) fn convert_to_local_tracked(
        self,
        timestamp: i64,
    ) -> Option<DateTime<Local>> {
        if matches!(timestamp, 0 | -1 | 1 | 11) {
            // For some reason these can be bad
            return None;
        }

        if !(1_000_000_000..=3_000_000_000).contains(&timestamp) {
            let caller = std::panic::Location::caller();
            warn!(
                "Weird timestamp: {timestamp} at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            );
            return None;
        }
        DateTime::from_timestamp(timestamp - self.0, 0)?
            .naive_utc()
            .and_local_timezone(Local)
            .latest()
    }

    /// Converts the raw timestamp from the server to the local time.
    #[must_use]
    #[allow(unused)]
    pub(crate) fn convert_to_local(
        self,
        timestamp: i64,
        name: &str,
    ) -> Option<DateTime<Local>> {
        if matches!(timestamp, 0 | -1 | 1 | 11) {
            // For some reason these can be bad
            return None;
        }

        if !(1_000_000_000..=3_000_000_000).contains(&timestamp) {
            warn!("Weird time stamp: {timestamp} for {name}");
            return None;
        }
        DateTime::from_timestamp(timestamp - self.0, 0)?
            .naive_utc()
            .and_local_timezone(Local)
            .latest()
    }

    /// The current time of the server in their time zone (whatever that might
    /// be). This uses the system time and calculates the offset to the
    /// servers time, so this is NOT the time at the last request, but the
    /// actual current time of the server.
    #[must_use]
    pub fn current(&self) -> NaiveDateTime {
        Local::now().naive_local() + Duration::seconds(self.0)
    }

    /// Returns the duration remaining until the next midnight on the server.
    #[must_use]
    pub fn next_midnight(&self) -> std::time::Duration {
        let current = self.current();
        let tomorrow = current.date() + Duration::days(1);
        let tomorrow = NaiveDateTime::from(tomorrow);
        let sec_until_midnight =
            (tomorrow - current).to_std().unwrap_or_default().as_secs();
        // Time stuff is weird so make sure this never skips a day + actual
        // amount
        std::time::Duration::from_secs(sec_until_midnight % (60 * 60 * 24))
    }
}

// https://stackoverflow.com/a/59955929
trait StringSetExt {
    fn set(&mut self, s: &str);
}

impl StringSetExt for String {
    /// Replace the contents of a string with a string slice. This is basically
    /// `self = s.to_string()`, but without the deallication of self +
    /// allocation of s for that
    fn set(&mut self, s: &str) {
        self.replace_range(.., s);
    }
}

/// The cost of something
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NormalCost {
    /// The amount of silver something costs
    pub silver: u64,
    /// The amount of mushrooms something costs
    pub mushrooms: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::AttributeType,
        gamestate::underworld::{UnderworldResourceType, UnderworldUnitType},
    };

    /// The whole `owntower` response: companions, underworld and ignored.
    #[test]
    fn test_update_tower() {
        let raw =
            "100001/0/100/360/1/0/0/4147/2539/2422/3743/2900/7153/2622/2391/\
             6428/4079/0/0/0/0/0/17534/481/951/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/416/5/56614/600/10/360/2/0/0/2422/2539/4147/\
             3743/2900/508/558/9704/5366/2203/0/0/0/0/0/4235/1244/2734/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/358/4/51450/600/2/\
             360/3/0/0/2422/4147/2539/3743/2900/1003/8878/910/7251/4199/0/0/0/\
             0/0/7117/582/1154/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/368/1/106984/600/0/0/15/15/29/15/15/15/15/15/15/15/0/1188000/\
             1188000/162518400/0/49500/1439888618/1460585058/25182501/\
             1786541817/0/0/0/504/0/2000/2000/80";
        let data = Vals::from(raw);
        assert_eq!(data.len(), TOWER_RESPONSE_LEN);

        let mut state = GameState::default();
        state.update_tower(&data, ServerTime::default()).unwrap();
        data.assert_all_read();

        let companions = state.dungeons.companions.as_ref().unwrap();
        let warrior = &companions[CompanionClass::Warrior];
        assert_eq!(warrior.level, 360);
        assert_eq!(warrior.attributes[AttributeType::Strength], 4147);
        assert_eq!(warrior.attributes[AttributeType::Luck], 2900);

        let scout = &companions[CompanionClass::Scout];
        assert_eq!(scout.attributes[AttributeType::Strength], 2422);
        assert_eq!(scout.attributes[AttributeType::Intelligence], 2539);

        let uw = state.underworld.as_ref().unwrap();
        assert_eq!(
            uw.buildings[UnderworldBuildingType::HeartOfDarkness].level,
            15
        );
        assert_eq!(uw.buildings[UnderworldBuildingType::GoldPit].level, 29);

        let goblin = &uw.units[UnderworldUnitType::Goblin];
        assert_eq!(goblin.upgraded_amount, 416);
        assert_eq!(goblin.count, 5);
        assert_eq!(goblin.total_attributes, 56_614);
        assert_eq!(goblin.level, 600);

        assert_eq!(uw.souls_limit, 162_518_400);
        assert_eq!(
            uw.production[UnderworldResourceType::Souls].per_hour,
            49_500
        );
        assert_eq!(uw.lure_level, 504);
        assert_eq!(uw.lured_today, 0);

        // No upgrade is running in this capture
        assert_eq!(uw.upgrade_building, None);
        assert_eq!(uw.upgrade_finish, None);
        assert!(uw.last_collectable_update.is_some());
    }

    #[test]
    fn test_update_character_status() {
        let raw = "1/1/1/1791552521/1791548921/11/6000/0/0/1791849601/0/0/0/1/\
                   0/2527/1386953147/0/0/0/1597743456/1790330859/1785000000/1/\
                   0/0/0/0/0/0/0/0/0";
        let data = Vals::from(raw);
        assert_eq!(data.len(), 33);

        let mut state = GameState::default();
        state
            .update_character_status(&data, ServerTime::default())
            .unwrap();
        data.assert_all_read();

        match state.tavern.current_action {
            tavern::CurrentAction::CityGuard { hours, .. } => {
                assert_eq!(hours, 1);
            }
            _ => panic!("expected CityGuard action"),
        }
        assert_eq!(state.tavern.beer_max, 11);
        assert_eq!(state.tavern.thirst_for_adventure_sec, 6000);
        assert_eq!(state.tavern.beer_drunk, 0);
        assert_eq!(state.specials.calendar.collected, 0);
        assert!(state.specials.calendar.next_possible.is_some());
        assert!(state.pets.as_ref().unwrap().next_free_exploration.is_some());
        assert!(state.dungeons.next_free_fight.is_some());
        assert!(
            state
                .legendary_dungeon
                .active
                .as_ref()
                .unwrap()
                .healing_start
                .is_some()
        );
    }

    #[test]
    fn test_update_resources() {
        let raw = "12345/150/987654/12/35/500000/1000000/250000/500000/15000/\
                   3200/88000/10/20/30/40/50";
        let data = Vals::from(raw);
        assert_eq!(data.len(), 17);

        let mut state = GameState::default();
        state.update_resources(&data);
        data.assert_all_read();

        assert_eq!(state.character.mushrooms, 150);
        assert_eq!(state.character.silver, 987_654);
        assert_eq!(state.specials.wheel.lucky_coins, 12);
        assert_eq!(state.tavern.quicksand_glasses, 35);

        let ft = state.fortress.as_ref().unwrap();
        assert_eq!(ft.resources[FortressResourceType::Wood].current, 500_000);
        assert_eq!(ft.resources[FortressResourceType::Stone].current, 250_000);

        let bs = state.blacksmith.as_ref().unwrap();
        assert_eq!(bs.metal, 15_000);
        assert_eq!(bs.arcane, 3200);

        let uw = state.underworld.as_ref().unwrap();
        assert_eq!(uw.souls_current, 88_000);

        let pets = state.pets.as_ref().unwrap();
        assert_eq!(pets.habitats[HabitatType::Shadow].fruits, 10);
        assert_eq!(pets.habitats[HabitatType::Light].fruits, 20);
        assert_eq!(pets.habitats[HabitatType::Earth].fruits, 30);
        assert_eq!(pets.habitats[HabitatType::Fire].fruits, 40);
        assert_eq!(pets.habitats[HabitatType::Water].fruits, 50);
    }

    #[test]
    fn test_update_user_settings() {
        let raw = "de/0/1/0/a/extra";
        let data = Vals::from(raw);
        let mut state = GameState::default();
        state.update_user_settings(&data);
        data.assert_all_read();

        assert_eq!(state.language, Some(Language::German));
        assert_eq!(
            state.tavern.questing_preference,
            ExpeditionSetting::PreferExpeditions
        );

        let raw2 = "en/0/0/0/b";
        let data2 = Vals::from(raw2);
        state.update_user_settings(&data2);
        data2.assert_all_read();

        assert_eq!(state.language, Some(Language::English));
        assert_eq!(
            state.tavern.questing_preference,
            ExpeditionSetting::PreferQuests
        );
    }

    #[test]
    fn test_update_character_save() {
        let mut parts = vec!["0".to_string(); 70];
        parts[1] = "12345".to_string(); // player_id
        parts[3] = "100".to_string(); // level
        parts[18] = "1".to_string(); // race (Human = 1)
        parts[20] = "1".to_string(); // class (Warrior = 1)
        parts[26] = "15".to_string(); // portal dmg bonus
        parts[28] = "25".to_string(); // portal hp bonus

        let raw = parts.join("/");
        let data = Vals::from(raw.as_str());
        let mut state = GameState::default();
        state
            .update_character_save(&data, ServerTime::default())
            .unwrap();
        data.assert_all_read();

        assert_eq!(state.character.player_id, 12345);
        assert_eq!(state.character.level, 100);
        assert_eq!(state.guild.unwrap().portal.damage_bonus, 15);
        assert_eq!(state.dungeons.portal.unwrap().player_hp_bonus, 25);
    }
}
