pub mod arena;
pub mod character;
pub mod dungeons;
pub mod event;
pub mod fortress;
pub mod guild;
pub mod idle;
pub mod items;
pub mod legendary_dungeon;
pub mod rewards;
pub mod social;
pub mod tavern;
pub mod underworld;
pub mod unlockables;

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use enum_map::EnumMap;
use log::{error, warn};
use num_traits::FromPrimitive;
use strum::{EnumCount, IntoEnumIterator};

use crate::{
    command::*,
    error::*,
    gamestate::{
        arena::*, character::*, dungeons::*, event::*, fortress::*, guild::*,
        idle::*, items::*, legendary_dungeon::*, rewards::*, social::*,
        tavern::*, underworld::*, unlockables::*,
    },
    misc::*,
    response::{Response, ResponseVal, Vals},
};

/// Represent the full state of the game at some point in time
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameState {
    /// Everything, that can be considered part of the character, or his
    /// immediate surrounding and not the rest of the world
    pub character: Character,
    /// Information about quests and work
    pub tavern: Tavern,
    /// The place to fight other players
    pub arena: Arena,
    /// The last fight, that this player was involved in
    pub last_fight: Option<Fight>,
    /// Both shops. You can access a specific one either with `get()`,
    /// `get_mut()`, or `[]` and the `ShopType` as the key.
    pub shops: EnumMap<ShopType, Shop>,
    pub shop_item_lvl: u32,
    /// If the player is in a guild, this will contain information about it
    pub guild: Option<Guild>,
    /// Everything, that is time sensitive, like events, calendar, etc.
    pub specials: TimedSpecials,
    /// Everything, that can be found under the Dungeon tab
    pub dungeons: Dungeons,
    /// Contains information about the underworld, if it has been unlocked
    pub underworld: Option<Underworld>,
    /// Contains information about the fortress, if it has been unlocked
    pub fortress: Option<Fortress>,
    /// Information the pet collection, that a player can build over time
    pub pets: Option<Pets>,
    /// Contains information about the hellevator, if it is currently active
    pub hellevator: HellevatorEvent,
    /// Contains information about the legendary dungeons event, if it is
    /// currently active
    pub legendary_dungeon: LegendaryDungeonEvent,
    /// Contains information about the blacksmith, if it has been unlocked
    pub blacksmith: Option<Blacksmith>,
    /// Contains information about the witch, if it has been unlocked
    pub witch: Option<Witch>,
    /// Tracker for small challenges, that a player can complete
    pub achievements: Achievements,
    /// The boring idle game
    pub idle_game: Option<IdleGame>,
    /// Contains the features this char is able to unlock right now
    pub pending_unlocks: Vec<Unlockable>,
    /// Anything related to hall of fames
    pub hall_of_fames: HallOfFames,
    /// Contains both other guilds & players, that you can look at via commands
    pub lookup: Lookup,
    /// Anything you can find in the mail tab of the official client
    pub mail: Mail,
    /// Information about an upcoming, or currently running event. Basically
    /// the tab above Tavern in the official GUI
    pub special_event: Option<EventStatus>,
    /// The current state of the world boss. Note that this here does not
    /// remove itself. You should always make sure `special_event` is ongoing
    /// before acting upon any world boss information
    pub world_boss: Option<WorldBossEvent>,
    /// The language, that the player has set in the client. This will affect
    /// the language of news messages in the inbox (Mail)
    pub language: Option<Language>,
    /// The raw timestamp, that the server has sent us
    last_request_timestamp: i64,
    /// The amount of sec, that the server is ahead of us in seconds (can be
    /// negative)
    server_time_diff: i64,
}

const SHOP_N: usize = 6;

/// A shop, that you can buy items from
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shop {
    pub typ: ShopType,
    /// The items this shop has for sale
    pub items: [Item; SHOP_N],
}

impl Default for Shop {
    fn default() -> Self {
        let items = core::array::from_fn(|_| Item {
            typ: ItemType::Unknown(0),
            price: u32::MAX,
            mushroom_price: u32::MAX,
            model_id: 0,
            class: None,
            type_specific_val: 0,
            attributes: EnumMap::default(),
            gem_slot: None,
            rune: None,
            enchantment: None,
            color: 0,
            upgrade_count: 0,
            item_quality: 0,
            is_washed: false,
            full_model_id: 0,
        });

        Self {
            items,
            typ: ShopType::Magic,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShopPosition {
    pub typ: ShopType,
    pub pos: usize,
}

impl std::fmt::Display for ShopPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.typ as usize, self.pos + 1)
    }
}

impl ShopPosition {
    /// The 0 based index into the backpack vec, where the item is parsed into
    #[must_use]
    pub fn shop(&self) -> ShopType {
        self.typ
    }
    /// The inventory type and position within it, where the item is stored
    /// according to previous inventory management logic. This is what you use
    /// for commands
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }
}

impl Shop {
    /// Creates an iterator over the inventory slots.
    pub fn iter(&self) -> impl Iterator<Item = (ShopPosition, &Item)> {
        self.items
            .iter()
            .enumerate()
            .map(|(pos, item)| (ShopPosition { typ: self.typ, pos }, item))
    }

    pub(crate) fn parse(
        data: &Vals<'_>,
        typ: ShopType,
    ) -> Result<Shop, SFError> {
        let mut shop = Shop::default();
        shop.typ = typ;
        for (chunk, item) in data.chunks(ITEM_PARSE_LEN).zip(&mut shop.items) {
            let Some(p_item) = Item::parse(&chunk)? else {
                return Err(SFError::ParsingError(
                    "shop item",
                    format!("{chunk:?}"),
                ));
            };
            *item = p_item;
        }
        Ok(shop)
    }
}

impl GameState {
    /// Constructs a new `GameState` from the provided response. The response
    /// has to be the login response from a `Session`.
    ///
    /// # Errors
    /// If the response contains any errors, or does not contain enough
    /// information about the player to build a full `GameState`, this will
    /// return a `ParsingError`, or `TooShortResponse` depending on the
    /// exact error
    pub fn new(response: Response) -> Result<Self, SFError> {
        let mut res = Self::default();
        res.update(response)?;
        if res.character.level == 0 || res.character.name.is_empty() {
            return Err(SFError::ParsingError(
                "response did not contain full player state",
                String::new(),
            ));
        }
        Ok(res)
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

        let mut other_player: Option<OtherPlayer> = None;
        let mut other_guild: Option<OtherGuild> = None;

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

    pub(crate) fn updatete_relation_list(&mut self, val: &str) {
        self.character.relations.clear();
        for entry in val
            .trim_end_matches(';')
            .split(';')
            .filter(|a| !a.is_empty())
        {
            let mut parts = entry.split(',');
            let (
                Some(id),
                Some(name),
                Some(guild),
                Some(level),
                Some(relation),
            ) = (
                parts.next().and_then(|a| a.parse().ok()),
                parts.next().map(std::string::ToString::to_string),
                parts.next().map(std::string::ToString::to_string),
                parts.next().and_then(|a| a.parse().ok()),
                parts.next().and_then(|a| match a {
                    "-1" => Some(Relationship::Ignored),
                    "1" => Some(Relationship::Friend),
                    _ => None,
                }),
            )
            else {
                warn!("bad friendslist entry: {entry}");
                continue;
            };
            self.character.relations.push(RelationEntry {
                id,
                name,
                guild,
                level,
                relation,
            });
        }
    }

    pub(crate) fn update_gttime(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let d = &mut self.hellevator;
        d.start = data.cstget(0, "event start", server_time)?;
        d.end = data.cstget(1, "event end", server_time)?;
        d.collect_time_end = data.cstget(3, "claim time end", server_time)?;
        Ok(())
    }

    pub(crate) fn update_tower(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        const BLOCK_OFFSET: usize = 3;

        data.ignore(0, "own player id");
        data.ignore(1, "always 0");
        data.ignore(2, "always 100");

        let companions = self.dungeons.companions.get_or_insert_default();
        for (i, class) in CompanionClass::iter().enumerate() {
            let start = BLOCK_OFFSET + i * underworld::TOWER_BLOCK_LEN;
            companions[class].update(
                &data.window(start..start + underworld::TOWER_BLOCK_LEN),
            );
        }

        let underworld = self.underworld.get_or_insert_default();
        for (i, unit) in underworld.units.values_mut().enumerate() {
            let start =
                BLOCK_OFFSET + (i + 1) * underworld::TOWER_BLOCK_LEN - 5;
            unit.update(&data.window(start..start + 4));
        }
        let tail =
            BLOCK_OFFSET + CompanionClass::COUNT * underworld::TOWER_BLOCK_LEN;
        underworld.update(
            &data.window(tail..underworld::TOWER_RESPONSE_LEN),
            server_time,
        );
        data.ignore(
            underworld::TOWER_RESPONSE_LEN..data.len(),
            "extra tower values",
        );
    }

    pub(crate) fn update_character_save(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.character.update(data, server_time);
        self.guild.get_or_insert_default().portal.damage_bonus =
            data.u8_or(26, 0);
        self.dungeons.portal.get_or_insert_default().player_hp_bonus =
            data.u16_or(28, 0);
    }

    pub(crate) fn update_character_status(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        data.ignore(0, "unknown character status value");
        self.tavern.current_action = CurrentAction::parse(
            data.i64_or(1, 0),
            data.i64_or(2, 0),
            data.server_time_opt(3, server_time),
        );
        // Index 4 contains the current action start.
        data.ignore(4, "current action start");

        self.tavern.beer_max = data.u8_or(5, 0);
        self.tavern.thirst_for_adventure_sec = data.u32_or(6, 0);
        self.tavern.beer_drunk = data.u8_or(7, 0);
        self.specials.calendar.collected = data.usize_or(8, 245);
        self.specials.calendar.next_possible =
            data.server_time_opt(9, server_time);
        data.ignore(10..20, "unmodelled character status values");

        self.pets.get_or_insert_default().next_free_exploration =
            data.server_time_opt(20, server_time);
        self.dungeons.next_free_fight = data.server_time_opt(21, server_time);

        let healing_start = data.server_time_opt(22, server_time);
        if let Some(start) = healing_start {
            self.legendary_dungeon
                .active
                .get_or_insert_default()
                .healing_start = Some(start);
        } else if let Some(active) = self.legendary_dungeon.active.as_mut() {
            active.healing_start = None;
        }

        data.ignore(23..data.len(), "extra character status values");
    }

    pub(crate) fn update_resources(
        &mut self,
        res: &[i64],
    ) -> Result<(), SFError> {
        self.character.mushrooms = res.csiget(1, "mushrooms", 0)?;
        self.character.silver = res.csiget(2, "player silver", 0)?;
        self.tavern.quicksand_glasses =
            res.csiget(4, "quicksand glass count", 0)?;

        self.specials.wheel.lucky_coins = res.csiget(3, "lucky coins", 0)?;
        let bs = self.blacksmith.get_or_insert_with(Default::default);
        bs.metal = res.csiget(9, "bs metal", 0)?;
        bs.arcane = res.csiget(10, "bs arcane", 0)?;
        let fortress = self.fortress.get_or_insert_with(Default::default);
        fortress
            .resources
            .get_mut(FortressResourceType::Wood)
            .current = res.csiget(5, "saved wood ", 0)?;
        fortress
            .resources
            .get_mut(FortressResourceType::Stone)
            .current = res.csiget(7, "saved stone", 0)?;

        let pets = self.pets.get_or_insert_with(Default::default);
        for (e_pos, element) in HabitatType::iter().enumerate() {
            pets.habitats.get_mut(element).fruits =
                res.csiget(12 + e_pos, "fruits", 0)?;
        }

        self.underworld
            .get_or_insert_with(Default::default)
            .souls_current = res.csiget(11, "uu souls saved", 0)?;
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
    fn get_fight(&mut self, header_name: &str) -> &mut SingleFight {
        let id = fight_no_from_header(header_name);
        let fights =
            &mut self.last_fight.get_or_insert_with(Default::default).fights;

        if fights.len() < id {
            fights.resize(id, SingleFight::default());
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
        other_player: &mut Option<OtherPlayer>,
        other_guild: &mut Option<OtherGuild>,
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
            "owngroupname" => self
                .guild
                .get_or_insert_with(Default::default)
                .name
                .set(val.as_str()),
            "sfhomeid" => {}
            "backpack" => {
                self.character.inventory.update(&val.list())?;
            }
            "itemlevelshop" => {
                self.shop_item_lvl = val.into("shop lvl")?;
            }
            "storeitemsshakes" => {
                *self.shops.get_mut(ShopType::Weapon) =
                    Shop::parse(&val.list(), ShopType::Weapon)?;
            }
            "questofferitems" => {
                let data = val.list();
                for (chunk, quest) in
                    data.chunks(ITEM_PARSE_LEN).zip(&mut self.tavern.quests)
                {
                    quest.item = Item::parse(&chunk)?;
                }
            }
            #[allow(
                clippy::indexing_slicing,
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation
            )]
            #[allow(deprecated)]
            "toiletstate" => {
                let vals: Vec<i64> = val.into_list("toilet state")?;
                if vals.len() < 3 {
                    return Ok(());
                }
                let toilet = self.tavern.toilet.get_or_insert_default();
                toilet.sacrifices_left = vals[2] as u32;
            }
            "companionequipment" => {
                let data = val.list();
                if data.is_empty() {
                    return Ok(());
                }
                for (idx, cmp) in self
                    .dungeons
                    .companions
                    .get_or_insert_with(Default::default)
                    .values_mut()
                    .enumerate()
                {
                    let start = EQUIPMENT_PARSE_LEN * idx;
                    cmp.equipment =
                        Equipment::parse(&data.window(start..data.len()))?;
                }
            }
            "storeitemsfidget" => {
                *self.shops.get_mut(ShopType::Magic) =
                    Shop::parse(&val.list(), ShopType::Magic)?;
            }
            "ownplayersaveequipment" => {
                self.character.equipment.update(&val.list())?;
            }
            "systemmessagelist" => {}
            "newslist" => {
                self.mail.news_inbox.clear();
                let data = val.as_str();
                for msg in data.split(';').filter(|a| !a.trim().is_empty()) {
                    match NewsEntry::parse(msg, server_time) {
                        Ok(msg) => self.mail.news_inbox.push(msg),
                        Err(e) => warn!("Invalid msg: {msg} {e}"),
                    }
                }
            }
            "dummieequipment" => {
                self.character.mannequin = Some(Equipment::parse(&val.list())?);
            }
            "owntower" => {
                self.update_tower(&val.list(), server_time);
            }
            "owngrouprank" => {
                self.guild.get_or_insert_with(Default::default).rank =
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
                    .get_or_insert_with(Default::default)
                    .update_group_save(&val.list(), server_time);
            }
            "owngroupmember" => self
                .guild
                .get_or_insert_with(Default::default)
                .update_member_names(&val.list_by(',')),
            "owngrouppotion" => {
                self.guild
                    .get_or_insert_with(Default::default)
                    .update_member_potions(&val.list_by(','));
            }
            "unitprice" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .update_unit_prices(&val.list());
            }
            "dicestatus" => {
                let dices: Option<Vec<DiceType>> = val
                    .into_list("dice status")?
                    .into_iter()
                    .map(FromPrimitive::from_u8)
                    .collect();
                self.tavern.dice_game.current_dice = dices.unwrap_or_default();
            }
            "dicereward" => {
                let data: Vec<u32> = val.into_list("dice reward")?;
                let win_typ: DiceType =
                    data.cfpuget(0, "dice reward", |a| a - 1)?;
                self.tavern.dice_game.reward = Some(DiceReward {
                    win_typ,
                    amount: data.cget(1, "dice reward amount")?,
                });
            }
            "chathistory" => {
                self.guild.get_or_insert_with(Default::default).chat =
                    ChatMessage::parse_messages(val.as_str());
            }
            "chatwhisper" => {
                self.guild.get_or_insert_with(Default::default).whispers =
                    ChatMessage::parse_messages(val.as_str());
            }
            "upgradeprice" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .update_unit_upgrade_info(&val.list());
            }
            "unitlevel" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .update_levels(&val.list());
            }
            "fortressprice" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .update_prices(&val.list());
            }
            "Arenarank" => {
                if let Some(uw) = self.underworld.as_mut() {
                    uw.lure_suggestion =
                        val.as_str().parse::<u32>().ok().map(LureSuggestion);
                }
            }
            "witch" => {
                // old witch data without price
            }
            "witchshop" => {
                self.witch
                    .get_or_insert_with(Default::default)
                    .update(&val.into_list("witch")?)?;
            }
            "underworldupgradeprice" => {
                self.underworld
                    .get_or_insert_with(Default::default)
                    .update_underworld_unit_prices(&val.list());
            }
            "unlockfeature" => {
                self.pending_unlocks =
                    Unlockable::parse(&val.into_list("unlock")?)?;
            }
            "dungeonprogresslight" => self.dungeons.update_progress(
                &val.into_list("dungeon progress light")?,
                DungeonType::Light,
            ),
            "dungeonprogressshadow" => self.dungeons.update_progress(
                &val.into_list("dungeon progress shadow")?,
                DungeonType::Shadow,
            ),
            "portalprogress" => {
                self.dungeons
                    .portal
                    .get_or_insert_with(Default::default)
                    .update(&val.into_list("portal progress")?, server_time)?;
            }
            "owntowerlevel" => {
                // Already in dungeons
            }
            "serverversion" => {
                // Handled in session
            }
            "stoneperhournextlevel" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .resources
                    .get_mut(FortressResourceType::Stone)
                    .production
                    .per_hour_next_lvl = val.into("stone next lvl")?;
            }
            "woodperhournextlevel" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .resources
                    .get_mut(FortressResourceType::Wood)
                    .production
                    .per_hour_next_lvl = val.into("wood next lvl")?;
            }
            "shadowlevel" | "dungeonlevel" => {
                // We just look at the db
            }
            "gttime" => {
                self.update_gttime(&val.into_list("gttime")?, server_time)?;
            }
            "gtsave" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .update(&val.into_list("gtsave")?, server_time)?;
            }
            "eventstatus" => {
                self.special_event =
                    Some(EventStatus::parse(&val.list(), server_time));
            }
            "wbcharacter" => {
                let data: Vec<i64> = val.into_list("wb char")?;

                self.world_boss
                    .get_or_insert_default()
                    .update(&data, server_time)?;
            }
            "wbtower" => {
                let data: Vec<String> = val.into_list("wb ug store")?;
                let data: Vec<_> = data.iter().map(|a| a.as_str()).collect();
                self.world_boss.get_or_insert_default().battle =
                    Some(WorldBossBattle::parse(&data)?);
            }
            "wbupgradestore" => {
                let data: Vec<i64> = val.into_list("wb ug store")?;
                let boss = self.world_boss.get_or_insert_default();
                if data.iter().all(|a| *a == 0) {
                    boss.upgrade_offers.clear();
                    return Ok(());
                }
                let offers = data
                    .chunks_exact(7)
                    .map(UpgradeOffer::parse)
                    .collect::<Result<_, SFError>>()?;
                boss.upgrade_offers = offers;
            }
            "wbammostore" => {
                let data: Vec<i64> = val.into_list("wb ammo store")?;
                let boss = self.world_boss.get_or_insert_default();
                if data.iter().all(|a| *a == 0) {
                    boss.projectile_offers.clear();
                    return Ok(());
                }
                let offers = data
                    .chunks_exact(6)
                    .map(ProjectileOffer::parse)
                    .collect::<Result<_, SFError>>()?;
                boss.projectile_offers = offers;
            }
            "wbammo" => {
                // Special ammo
                let data: Vec<i64> = val.into_list("wb ammo")?;
                let ammo = WorldBossProjectile::parse(&data)?;
                self.world_boss.get_or_insert_default().projectile = ammo;
            }
            "wbupgrade" => {
                let data: Vec<i64> = val.into_list("wb upgrade")?;
                self.world_boss.get_or_insert_default().catapult =
                    WorldBossCatapult::parse(&data, server_time)?;
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
                let data: Vec<u32> = val.into_list("wb upgrade")?;
                let chests = &mut self
                    .world_boss
                    .get_or_insert_default()
                    .available_daily_chests;
                for (old, new) in chests.values_mut().zip(data) {
                    *old = new;
                }
            }
            "maxrank" => {
                self.hall_of_fames.players_total = val.into("player count")?;
            }
            "achievement" => {
                self.achievements.update(&val.into_list("achievements")?)?;
            }
            "groupskillprice" => {
                self.guild
                    .get_or_insert_with(Default::default)
                    .update_group_prices(&val.list());
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
                // Replaced
            }
            "owngroupdescription" => self
                .guild
                .get_or_insert_with(Default::default)
                .update_description_embed(val.as_str()),
            "idle" => {
                self.idle_game = IdleGame::parse_idle_game(
                    &val.into_list("idle game")?,
                    server_time,
                );
            }
            "resources" => {
                self.update_resources(&val.into_list("resources")?)?;
            }
            "chattime" => {
                // let _chat_time = server_time
                //     .convert_to_local(val.into("chat time")?, "chat
                // time"); Pretty sure this is the time something last
                // happened in chat, but nobody cares and messages have a
                // time
            }
            "maxpetlevel" => {
                self.pets.get_or_insert_with(Default::default).max_pet_level =
                    val.into("max pet lvl")?;
            }
            "otherdescription" => {
                other_player
                    .get_or_insert_with(Default::default)
                    .description = from_sf_string(val.as_str());
            }
            "otherplayergroupname" => {
                let guild =
                    Some(val.as_str().to_string()).filter(|a| !a.is_empty());
                other_player.get_or_insert_with(Default::default).guild = guild;
            }
            "otherplayername" => {
                other_player
                    .get_or_insert_with(Default::default)
                    .name
                    .set(val.as_str());
            }
            "otherplayersaveequipment" => {
                other_player.get_or_insert_with(Default::default).equipment =
                    Equipment::parse(&val.list())?;
            }
            "fortresspricereroll" => {
                let data = val.list();
                self.fortress
                    .get_or_insert_with(Default::default)
                    .opponent_reroll_price = data.u64_or(0, 0);
            }
            "fortresswalllevel" => {
                let data = val.list();
                self.fortress
                    .get_or_insert_with(Default::default)
                    .wall_combat_lvl = data.u16_or(0, 0);
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
                self.specials.wheel.result = Some(WheelReward::parse(
                    &val.into_list("wheel result")?,
                    upgraded,
                )?);
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
                // Contains language and flag settings
                let vals = &val.as_str().split('/').collect::<Vec<_>>();
                self.language = Language::parse(vals.cget(0, "client lang")?);

                let v = match vals.cget(4, "questing setting")? {
                    "a" => ExpeditionSetting::PreferExpeditions,
                    "0" | "b" => ExpeditionSetting::PreferQuests,
                    x => {
                        error!("Weird expedition settings: {x}");
                        ExpeditionSetting::PreferQuests
                    }
                };
                self.tavern.questing_preference = v;
            }
            "mailinvoice" => {
                // Incomplete email address
            }
            "calenderinfo" => {
                // This is twice in the original response.
                // This API sucks LMAO
                let data: Vec<i64> = val.into_list("calendar")?;
                self.specials.calendar.rewards.clear();
                for p in data.chunks_exact(2) {
                    let reward = CalendarReward::parse(p)?;
                    self.specials.calendar.rewards.push(reward);
                }
            }
            "othergroupattack" => {
                other_guild.get_or_insert_with(Default::default).attacks =
                    Some(val.to_string());
            }
            "othergroupdefense" => {
                other_guild
                    .get_or_insert_with(Default::default)
                    .defends_against = Some(val.to_string());
            }
            "inboxcapacity" => {
                self.mail.inbox_capacity = val.into("inbox cap")?;
            }
            "magicregistration" => {
                // Pretty sure this means you have not provided a pw or
                // mail. Just a name and clicked play
            }
            "Ranklistplayer" => {
                self.hall_of_fames.players.clear();
                for player in val.as_str().trim_matches(';').split(';') {
                    // Stop parsing once we receive an empty player
                    if player.ends_with(",,,0,0,0,") {
                        break;
                    }

                    match HallOfFamePlayer::parse(player) {
                        Ok(x) => {
                            self.hall_of_fames.players.push(x);
                        }
                        Err(err) => warn!("{err}"),
                    }
                }
            }
            "ranklistgroup" => {
                self.hall_of_fames.guilds.clear();
                for guild in val.as_str().trim_matches(';').split(';') {
                    match HallOfFameGuild::parse(guild) {
                        Ok(x) => {
                            self.hall_of_fames.guilds.push(x);
                        }
                        Err(err) => warn!("{err}"),
                    }
                }
            }
            "maxrankgroup" => {
                self.hall_of_fames.guilds_total = Some(val.into("guild max")?);
            }
            "maxrankPets" => {
                self.hall_of_fames.pets_total = Some(val.into("pet rank max")?);
            }
            "RanklistPets" => {
                self.hall_of_fames.pets.clear();
                for entry in val.as_str().trim_matches(';').split(';') {
                    match HallOfFamePets::parse(entry) {
                        Ok(x) => {
                            self.hall_of_fames.pets.push(x);
                        }
                        Err(err) => warn!("{err}"),
                    }
                }
            }
            "ranklistfortress" | "Ranklistfortress" => {
                self.hall_of_fames.fortresses.clear();
                for guild in val.as_str().trim_matches(';').split(';') {
                    match HallOfFameFortress::parse(guild) {
                        Ok(x) => {
                            self.hall_of_fames.fortresses.push(x);
                        }
                        Err(err) => warn!("{err}"),
                    }
                }
            }
            "ranklistunderworld" => {
                self.hall_of_fames.underworlds.clear();
                for entry in val.as_str().trim_matches(';').split(';') {
                    match HallOfFameUnderworld::parse(entry) {
                        Ok(x) => {
                            self.hall_of_fames.underworlds.push(x);
                        }
                        Err(err) => warn!("{err}"),
                    }
                }
            }
            "gamblegoldvalue" => {
                self.tavern.gamble_result =
                    Some(GambleResult::SilverChange(val.into("gold gamble")?));
            }
            "gamblecoinvalue" => {
                self.tavern.gamble_result = Some(GambleResult::MushroomChange(
                    val.into("gold gamble")?,
                ));
            }
            "maxrankFortress" => {
                self.hall_of_fames.fortresses_total =
                    Some(val.into("fortress max")?);
            }
            "underworldprice" => self
                .underworld
                .get_or_insert_with(Default::default)
                .update_building_prices(&val.list()),
            "owngroupknights" => self
                .guild
                .get_or_insert_with(Default::default)
                .update_group_knights(&val.list_by(',')),
            "friendlist" => self.updatete_relation_list(val.as_str()),
            "legendaries" => {
                if val.as_str().chars().any(|a| a != 'A') {
                    warn!("Found a legendaries value, that is not just AAA..");
                }
            }
            "smith" => {
                let data: Vec<i64> = val.into_list("smith")?;
                let bs = self.blacksmith.get_or_insert_with(Default::default);

                bs.dismantle_left = data.csiget(0, "dismantles left", 0)?;
                bs.last_dismantled = data.cstget(1, "bs time", server_time)?;
            }
            "fortressGroupPrice" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .hall_of_knights_upgrade_price =
                    FortressCost::parse(&val.list());
            }
            "goldperhournextlevel" => {
                // I dont think this matters
            }
            "underworldmaxsouls" => {
                // This should already be in resources
            }
            "dailytaskrewardpreview" => {
                let vals: Vec<i64> =
                    val.into_list("event task reward preview")?;
                self.specials.tasks.daily.rewards = parse_rewards(&vals);
            }
            "expeditionevent" => {
                let data: Vec<i64> = val.into_list("exp event")?;
                self.tavern.expeditions.start =
                    data.cstget(0, "expedition start", server_time)?;
                let end = data.cstget(1, "expedition end", server_time)?;
                self.tavern.expeditions.end = end;
            }
            "expeditions" => {
                let data: Vec<i64> = val.into_list("exp event")?;

                if !data.len().is_multiple_of(8) {
                    warn!(
                        "Available expeditions have weird size: {data:?} {}",
                        data.len()
                    );
                }
                self.tavern.expeditions.available = data
                    .chunks_exact(8)
                    .map(|data| {
                        Ok(AvailableExpedition {
                            target: data
                                .cfpget(0, "expedition typ", |a| a)?
                                .unwrap_or_default(),
                            location_1: data
                                .cfpget(4, "exp loc 1", |a| a)?
                                .unwrap_or_default(),
                            location_2: data
                                .cfpget(5, "exp loc 2", |a| a)?
                                .unwrap_or_default(),
                            thirst_for_adventure_sec: data
                                .csiget(6, "exp alu", 600)?,
                            special: data.cfpget(7, "exp special", |a| a)?,
                        })
                    })
                    .collect::<Result<_, _>>()?;
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
                let data: Vec<i64> = val.into_list("expedition monster")?;
                let exp = self
                    .tavern
                    .expeditions
                    .active
                    .get_or_insert_with(Default::default);

                exp.boss = ExpeditionBoss {
                    id: data
                        .cfpget(0, "expedition monster", |a| -a)?
                        .unwrap_or_default(),
                    items: soft_into(
                        data.get(1).copied().unwrap_or_default(),
                        "exp monster items",
                        3,
                    ),
                };
            }
            "expeditionhalftime" => {
                let data: Vec<i64> = val.into_list("halftime exp")?;
                let exp = self
                    .tavern
                    .expeditions
                    .active
                    .get_or_insert_with(Default::default);

                exp.halftime_for_boss_id =
                    -data.cget(0, "halftime for boss id")?;
                exp.rewards = data
                    .skip(1, "halftime choice")?
                    .chunks_exact(2)
                    .map(Reward::parse)
                    .collect::<Result<_, _>>()?;
            }
            "expeditionstate" => {
                let data: Vec<i64> = val.into_list("exp state")?;
                let exp = self
                    .tavern
                    .expeditions
                    .active
                    .get_or_insert_with(Default::default);
                exp.floor_stage = data.cget(2, "floor stage")?;

                exp.target_thing = data
                    .cfpget(3, "expedition target", |a| a)?
                    .unwrap_or_default();
                exp.target_current = data.csiget(7, "exp current", 100)?;
                exp.target_amount = data.csiget(8, "exp target", 100)?;

                exp.current_floor = data.csiget(0, "clearing", 0)?;
                exp.heroism = data.csiget(13, "heroism", 0)?;

                exp.busy_since = data.cstget(15, "exp start", server_time)?;
                exp.busy_until = data.cstget(16, "exp busy", server_time)?;

                for (x, item) in data
                    .skip(9, "exp items")?
                    .iter()
                    .copied()
                    .zip(&mut exp.items)
                {
                    *item = match FromPrimitive::from_i64(x) {
                        None if x != 0 => {
                            warn!("Unknown item: {x}");
                            Some(ExpeditionThing::Unknown)
                        }
                        x => x,
                    };
                }
            }
            "expeditioncrossroad" => {
                // 3/3/132/0/2/2
                let data: Vec<i64> = val.into_list("cross")?;
                let exp = self
                    .tavern
                    .expeditions
                    .active
                    .get_or_insert_with(Default::default);
                exp.update_encounters(&data);
            }
            "eventtasklist" => {
                let data: Vec<i64> = val.into_list("etl")?;
                self.specials.tasks.event.tasks.clear();
                for c in data.chunks_exact(4) {
                    let task = Task::parse(c)?;
                    self.specials.tasks.event.tasks.push(task);
                }
            }
            "eventtaskrewardpreview" => {
                let vals: Vec<i64> =
                    val.into_list("event task reward preview")?;

                self.specials.tasks.event.rewards = parse_rewards(&vals);
            }
            "dailytasklist" => {
                let data: Vec<i64> = val.into_list("daily tasks list")?;
                self.specials.tasks.daily.tasks.clear();

                // I think the first value here is the amount of > 1 bell
                // quests
                for d in data.skip(1, "daily tasks")?.chunks_exact(4) {
                    self.specials.tasks.daily.tasks.push(Task::parse(d)?);
                }
            }
            "eventtaskinfo" => {
                let data: Vec<i64> = val.into_list("eti")?;
                self.specials.tasks.event.theme = data
                    .cfpget(2, "event task theme", |a| a)?
                    .unwrap_or(EventTaskTheme::Unknown);
                self.specials.tasks.event.start =
                    data.cstget(0, "event t start", server_time)?;
                self.specials.tasks.event.end =
                    data.cstget(1, "event t end", server_time)?;
            }
            "scrapbook" => {
                self.character.scrapbook = ScrapBook::parse(val.as_str());
            }
            "dungeonfaces" | "shadowfaces" => {
                // Gets returned after winning a dungeon fight. This looks a
                // bit like a reward, but that should be handled in fight
                // parsing already?
            }
            "messagelist" => {
                let data = val.as_str();
                self.mail.inbox.clear();
                for msg in data.split(';').filter(|a| !a.trim().is_empty()) {
                    match InboxEntry::parse(msg, server_time) {
                        Ok(msg) => self.mail.inbox.push(msg),
                        Err(e) => warn!("Invalid msg: {msg} {e}"),
                    }
                }
            }
            "messagetext" => {
                self.mail.open_msg = Some(from_sf_string(val.as_str()));
            }
            "combatloglist" => {
                self.mail.combat_log.clear();
                for entry in val.as_str().split(';') {
                    let parts = entry.split(',').collect::<Vec<_>>();
                    if parts.iter().all(|a| a.is_empty()) {
                        continue;
                    }
                    match CombatLogEntry::parse(&parts, server_time) {
                        Ok(cle) => {
                            self.mail.combat_log.push(cle);
                        }
                        Err(e) => {
                            warn!(
                                "Unable to parse combat log entry: {parts:?} \
                                 - {e}"
                            );
                        }
                    }
                }
            }
            "maxupgradelevel" => {
                self.fortress
                    .get_or_insert_with(Default::default)
                    .building_max_lvl = val.into("max upgrade lvl")?;
            }
            "singleportalenemylevel" => {
                self.dungeons
                    .portal
                    .get_or_insert_with(Default::default)
                    .enemy_level = val.into("portal lvl").unwrap_or(u32::MAX);
            }
            "ownpetsstats" => {
                self.pets
                    .get_or_insert_with(Default::default)
                    .update_pet_stat(&val.into_list("pet stats")?);
            }
            "ownpets" => {
                let data = val.into_list("own pets")?;
                self.pets
                    .get_or_insert_with(Default::default)
                    .update(&data, server_time)?;
            }
            "petsdefensetype" => {
                let pet_id = val.into("pet def typ")?;
                self.pets
                    .get_or_insert_with(Default::default)
                    .opponent
                    .habitat = Some(HabitatType::from_typ_id(pet_id).ok_or(
                    SFError::ParsingError("pet def typ", format!("{pet_id}")),
                )?);
            }
            "otherplayersavecharacter" => {
                other_player
                    .get_or_insert_default()
                    .update(&val.into_list("other player")?, server_time)?;
            }
            "otherplayersavepotions" => {
                other_player.get_or_insert_default().active_potions =
                    items::parse_active_potions(
                        &val.into_list("other potions")?,
                        server_time,
                    );
            }
            "otherplayer" => {
                let data: Vec<i64> = val.into_list("other player")?;
                #[allow(deprecated)]
                {
                    other_player.get_or_insert_default().guild_joined =
                        data.cstget(166, "other joined guild", server_time)?;
                }
            }
            "otherplayerfriendstatus" => {
                other_player
                    .get_or_insert_with(Default::default)
                    .relationship = warning_parse(
                    val.into::<i32>("other friend")?,
                    "other friend",
                    FromPrimitive::from_i32,
                )
                .unwrap_or_default();
            }
            "otherplayerpetbonus" => {
                other_player
                    .get_or_insert_with(Default::default)
                    .update_pet_bonus(&val.into_list("o pet bonus")?)?;
            }
            "otherplayerunitlevel" => {
                let data: Vec<i64> =
                    val.into_list("other player unit level")?;
                // This includes other levels, but they are handled
                // elsewhere I think
                other_player
                    .get_or_insert_with(Default::default)
                    .wall_combat_lvl = data.csiget(0, "wall_lvl", 0)?;
            }
            "petsrank" => {
                self.pets.get_or_insert_with(Default::default).rank =
                    val.into("pet rank")?;
            }

            "maxrankUnderworld" => {
                self.hall_of_fames.underworlds_total =
                    Some(val.into("mrank under")?);
            }
            "otherplayerfortressrank" => {
                match val.into::<i64>("other player fortress rank")? {
                    ..=-1 => {}
                    x => {
                        let rank = x.try_into().unwrap_or(1);
                        other_player
                            .get_or_insert_default()
                            .fortress
                            .get_or_insert_default()
                            .rank = rank;
                    }
                }
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
                self.last_fight
                    .get_or_insert_with(Default::default)
                    .update_result(&val.list())?;
                // Note: The sub_key from this, can improve fighter parsing
            }
            x if x.starts_with("fightheader") => {
                self.get_fight(x).update_fighters(&val.list());
            }
            "fightgroups" => {
                let fight =
                    self.last_fight.get_or_insert_with(Default::default);
                fight.update_groups(&val.list_by(','));
            }
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
                let fight = self.get_fight(x);
                fight.update_rounds(&val.list(), fight_version);
            }
            "othergroupname" => {
                other_guild
                    .get_or_insert_with(Default::default)
                    .name
                    .set(val.as_str());
            }
            "othergrouprank" => {
                other_guild.get_or_insert_with(Default::default).rank =
                    val.into("other group rank")?;
            }
            "othergroupfightcost" => {
                other_guild.get_or_insert_with(Default::default).attack_cost =
                    val.into("other group fighting cost")?;
            }
            "othergroupmember" => {
                let names: Vec<_> = val.as_str().split(',').collect();
                let og = other_guild.get_or_insert_with(Default::default);
                og.members.resize_with(names.len(), Default::default);
                for (m, n) in og.members.iter_mut().zip(names) {
                    m.name.set(n);
                }
            }
            "othergroupdescription" => {
                let guild = other_guild.get_or_insert_with(Default::default);
                let (emblem, desc) =
                    val.as_str().split_once('§').unwrap_or(("", val.as_str()));

                guild.emblem.update(emblem);
                guild.description = from_sf_string(desc);
            }
            "othergroup" => {
                other_guild
                    .get_or_insert_with(Default::default)
                    .update(val.as_str(), server_time)?;
            }
            "reward" => {
                // This is the task reward, which you should already know
                // from collecting
            }
            "gtdailypoints" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
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
                let all: Vec<_> = val.as_str().split('/').collect();
                let hellevator =
                    self.hellevator.active.get_or_insert_with(Default::default);

                for floor in &mut hellevator.guild_raid_floors {
                    floor.today_assigned.clear();
                }

                #[allow(clippy::indexing_slicing)]
                for part in all.chunks_exact(2) {
                    // The name of the guild member
                    let name = part[0];
                    // should be the dungeon they signed up for today
                    let val: usize = part
                        .cget(1, "hell raid part")
                        .ok()
                        .and_then(|a| a.parse().ok())
                        .unwrap_or(0);
                    if val > 0 {
                        if val > hellevator.guild_raid_floors.len() {
                            hellevator
                                .guild_raid_floors
                                .resize_with(val, Default::default);
                        }
                        if let Some(floor) =
                            hellevator.guild_raid_floors.get_mut(val - 1)
                        {
                            floor.today_assigned.push(name.to_string());
                        }
                    }
                }
            }
            "gtraidparticipantsyesterday" => {
                let all: Vec<_> = val.as_str().split('/').collect();

                let hellevator =
                    self.hellevator.active.get_or_insert_with(Default::default);

                for floor in &mut hellevator.guild_raid_floors {
                    floor.yesterday_assigned.clear();
                }

                #[allow(clippy::indexing_slicing)]
                for part in all.chunks_exact(2) {
                    // The name of the guild member
                    let name = part[0];
                    // should be the dungeon they signed up for today
                    let val: usize = part
                        .cget(1, "hell raid part yd")
                        .ok()
                        .and_then(|a| a.parse().ok())
                        .unwrap_or(0);
                    if val > 0 {
                        if val > hellevator.guild_raid_floors.len() {
                            hellevator
                                .guild_raid_floors
                                .resize_with(val, Default::default);
                        }
                        if let Some(floor) =
                            hellevator.guild_raid_floors.get_mut(val - 1)
                        {
                            floor.yesterday_assigned.push(name.to_string());
                        }
                    }
                }
            }
            "gtrank" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .guild_rank = val.into("gt rank").unwrap_or(0);
            }
            "gtrankingmax" => {
                self.hall_of_fames.hellevator_total =
                    val.into("gt rank max").ok();
            }
            "gtbracketlist" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .brackets =
                    val.into_list("gtbracketlist").unwrap_or_default();
            }
            "gtraidfights" => {
                let data: Vec<i64> =
                    val.into_list("gt raids").unwrap_or_default();

                let hellevator =
                    self.hellevator.active.get_or_insert_with(Default::default);

                hellevator.guild_raid_signup_start = data
                    .cstget(0, "h raid signup start", server_time)?
                    .unwrap_or_default();

                hellevator.guild_raid_start = data
                    .cstget(1, "h raid next attack", server_time)?
                    .unwrap_or_default();

                let start = data.skip(2, "hellevator_fights")?;

                let floor_count = start.len() / 5;

                if floor_count > hellevator.guild_raid_floors.len() {
                    hellevator
                        .guild_raid_floors
                        .resize_with(floor_count, Default::default);
                }
                #[allow(clippy::indexing_slicing)]
                for (data, floor) in
                    start.chunks_exact(5).zip(&mut hellevator.guild_raid_floors)
                {
                    // FIXME: What are these?
                    floor.today = data[1];
                    floor.yesterday = data[2];
                    floor.point_reward =
                        data.csiget(3, "floor t-reward", 0).unwrap_or(0);
                    floor.silver_reward =
                        data.csiget(4, "floor c-reward", 0).unwrap_or(0);
                }
            }
            "gtmonsterreward" => {
                let data: Vec<i64> =
                    val.into_list("gt m reward").unwrap_or_default();

                let hellevator =
                    self.hellevator.active.get_or_insert_with(Default::default);
                hellevator.monster_rewards.clear();

                for chunk in data.chunks_exact(3) {
                    let raw_typ = chunk.cget(0, "gt monster reward typ")?;
                    if raw_typ <= 0 {
                        continue;
                    }
                    let one = chunk
                        .csiget(1, "gt monster reward typ", 0)
                        .unwrap_or(0);
                    if one != 0 {
                        warn!("hellevator monster t: {one}");
                    }
                    let typ = HellevatorMonsterRewardTyp::parse(raw_typ);
                    let amount: u64 =
                        chunk.csiget(2, "gt monster reward amount", 0)?;
                    hellevator
                        .monster_rewards
                        .push(HellevatorMonsterReward { typ, amount });
                }
            }
            "gtdailyreward" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .rewards_today = HellevatorDailyReward::parse(
                    &val.into_list("hdrtd").unwrap_or_default(),
                );
            }
            "gtdailyrewardnext" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .rewards_next = HellevatorDailyReward::parse(
                    &val.into_list("hdrnd").unwrap_or_default(),
                );
            }
            "gtdailyrewardyesterday" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .rewards_yesterday = HellevatorDailyReward::parse(
                    &val.into_list("hdryd").unwrap_or_default(),
                );
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
                self.hall_of_fames.hellevator = val
                    .as_str()
                    .split(';')
                    .filter(|a| !a.is_empty())
                    .map(|chunk| chunk.split(',').collect())
                    .flat_map(|chunk: Vec<_>| -> Result<_, SFError> {
                        Ok(HallOfFameHellevator {
                            rank: chunk.cfsuget(0, "hh rank")?,
                            name: chunk.cget(1, "hh name")?.to_string(),
                            tokens: chunk.cfsuget(2, "hh tokens")?,
                        })
                    })
                    .collect();
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
                    .get_or_insert_with(Default::default)
                    .current_monster = HellevatorMonster::parse(
                    &val.into_list("h monster").unwrap_or_default(),
                )
                .ok();
            }
            "gtbonus" => {
                self.hellevator
                    .active
                    .get_or_insert_with(Default::default)
                    .daily_treat_bonus = val
                    .into_list("gt bonus")
                    .and_then(|a| HellevatorTreatBonus::parse(&a))
                    .ok();
            }
            "pendingrewards" => {
                let vals: Vec<_> = val.as_str().split('/').collect();
                self.mail.claimables = vals
                    .chunks_exact(6)
                    .flat_map(|chunk| -> Result<ClaimableMail, SFError> {
                        let start = chunk.cfsuget(4, "p reward start")?;
                        let end = chunk.cfsuget(5, "p reward end")?;

                        let status = match chunk.cget(1, "p read")? {
                            "0" => ClaimableStatus::Unread,
                            "1" => ClaimableStatus::Read,
                            "2" => ClaimableStatus::Claimed,
                            x => {
                                warn!("Unknown claimable status: {x}");
                                ClaimableStatus::Claimed
                            }
                        };

                        Ok(ClaimableMail {
                            typ: FromPrimitive::from_i64(
                                chunk.cfsuget(2, "claimable typ")?,
                            )
                            .unwrap_or_default(),
                            msg_id: chunk.cfsuget(0, "msg_id")?,
                            status,
                            name: chunk.cget(3, "reward code")?.to_string(),
                            received: server_time
                                .convert_to_local(start, "p start"),
                            claimable_until: server_time
                                .convert_to_local(end, "p end"),
                        })
                    })
                    .collect();
            }
            "pendingrewardressources" => {
                let vals: Vec<i64> =
                    val.into_list("pendingrewardressources")?;

                self.mail
                    .open_claimable
                    .get_or_insert_with(Default::default)
                    .resources = vals
                    .chunks_exact(2)
                    .flat_map(|chunk| -> Result<Reward, SFError> {
                        Ok(Reward {
                            typ: RewardType::parse(chunk.cget(0, "c typ")?),
                            amount: chunk.csiget(1, "c amount", 1)?,
                        })
                    })
                    .collect();
            }
            "pendingreward" => {
                let data = val.list();
                self.mail
                    .open_claimable
                    .get_or_insert_with(Default::default)
                    .items = data
                    .chunks(ITEM_PARSE_LEN)
                    .filter_map(|chunk| Item::parse(&chunk).ok().flatten())
                    .collect();
            }
            "fightablegroups" => {
                self.guild
                    .get_or_insert_default()
                    .update_fightable_targets(&val.list())?;
            }
            "adventscalendar" => {
                let vals: Vec<i64> = val.into_list("advent door")?;
                self.specials.advent_calendar = match vals.first() {
                    Some(0) | None => None,
                    _ => Reward::parse(&vals).ok(),
                };
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
                    .update_resources(&val.list(), server_time);
            }
            "fortressunits" => {
                self.fortress
                    .get_or_insert_default()
                    .update_units(&val.list(), server_time);
            }
            "fortress" => {
                self.fortress
                    .get_or_insert_default()
                    .update(&val.list(), server_time);
            }
            "wheel" => {
                let data: Vec<i64> = val.into_list("wheel")?;
                // [0] => 2 ??
                self.specials.wheel.spins_today =
                    data.csiget(1, "lucky turns", 0)?;
                self.specials.wheel.next_free_spin =
                    data.cstget(2, "next lucky turn", server_time)?;
            }
            "dice" => {
                let data: Vec<i64> = val.into_list("dice")?;
                self.tavern.dice_game.next_free =
                    data.cstget(0, "dice next", server_time)?;
                self.tavern.dice_game.remaining =
                    data.csiget(1, "rem dice games", 0)?;
            }
            "charactergroup" => {
                self.guild
                    .get_or_insert_with(Default::default)
                    .update_character_group(&val.list(), server_time);
            }
            "arena" => {
                self.arena.update(&val.list(), server_time);
            }
            "ownplayersavepotions" => {
                let data: Vec<i64> = val.into_list("potions")?;
                self.character.active_potions =
                    items::parse_active_potions(&data, server_time);
            }
            "arcanetoilet" => {
                let data: Vec<i64> = val.into_list("toilet")?;

                // Toilet remains none as long as its level is 0
                let toilet_lvl = data.cget(0, "toilet lvl")?;
                if toilet_lvl > 0 {
                    self.tavern
                        .toilet
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time)?;
                }
            }
            "vipstatus" => {
                other_player.get_or_insert_default().is_vip =
                    val.as_str() != "0";
            }
            "characterstatus" => {
                self.update_character_status(&val.list(), server_time);
            }
            "ownplayersavecharacter" => {
                self.update_character_save(&val.list(), server_time);
            }
            "adventure" => {
                let data: Vec<i64> = val.into_list("char save")?;
                // 198 - Might be the person to give you the quest?
                // 198 - ??
                for (slice, quest) in data
                    .skip(2, "quests")?
                    .chunks_exact(7)
                    .zip(&mut self.tavern.quests)
                {
                    quest.update(slice)?;
                }
            }
            "events" => {
                self.specials.events.update(val.as_str(), server_time)?;
            }
            "tavernspecialend" | "tavernspecialsub" | "tavernspecial" => {
                // Removed old way to serve events
            }
            "subscriptionstatus" => {}
            // Legendary Dungeons
            "iadungeonchances" => {
                // IDK
                val.list().ignore_all("drop chances are not modelled");
            }
            "iadungeontime" => {
                self.legendary_dungeon.update_time(&val.list(), server_time);
            }
            "iadungeonstatstotal" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.total_stats = TotalStats::parse(&val.list());
            }
            "iadungeonstats" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.stats = Stats::parse(&val.list());
            }
            "iadungeon" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.update(&val.list());
                if !all_values.contains_key("iapendingitems") {
                    dungeons.pending_items.clear();
                }
            }
            "iapendingitems" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_pending_items(&val.list());
            }
            "ialootitem" => {
                // The stuff, that was looted after a fight
                val.list().ignore_all("loot is already handled elsewhere");
            }
            "iamerchant" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_merchant(&val.list());
            }
            "iadungeon20cost" => {
                let data = val.list();
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .heal_quarter_cost = data.u32_or(0, 0);
                data.ignore(1..data.len(), "extra healing cost values");
            }
            "iadungeonsoulstones" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .update_soul_stones(&val.list());
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
                val.list().ignore_all("map layout is not needed for gems");
            }
            "otherplayerfortressinfo" => {
                other_player
                    .get_or_insert_default()
                    .update_fortress(&val.into_list("other ft")?)?;
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
                // from regular Item — first value is model_id, not type)
            }
            x if x.starts_with("externaltoolequipment") => {
                // External tool/mount equipment data. Format unknown.
            }
            x if x.starts_with("fightdecoration") => {
                // Cosmetic decoration data. Not currently parsed.
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
    /// Converts the raw timestamp from the server to the local time.
    #[must_use]
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
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::needless_range_loop
    )]

    use super::*;

    fn character_save_values() -> Vec<String> {
        let mut values = vec!["0".to_owned(); 70];
        values[0] = "ignored".to_owned();
        values[1] = "123456".to_owned();
        values[2] = "ignored".to_owned();
        values[3] = "1048618".to_owned();
        values[4] = "9000000000".to_owned();
        values[5] = "12345678".to_owned();
        values[6] = "4321".to_owned();
        values[7] = "17".to_owned();
        values[8] = "4".to_owned();
        values[9] = "206".to_owned();
        values[10] = "103".to_owned();
        values[11] = "2".to_owned();
        values[12] = "104".to_owned();
        values[13] = "5".to_owned();
        values[14] = "6".to_owned();
        values[15] = "7".to_owned();
        values[16] = "108".to_owned();
        values[17] = "9".to_owned();
        values[18] = "3".to_owned();
        values[19] = "1".to_owned();
        values[20] = "4".to_owned();
        values[21] = "260".to_owned();
        values[22] = "ignored".to_owned();
        values[23] = "98765".to_owned();
        values[24] = "123".to_owned();
        values[25] = "456".to_owned();
        values[26] = "12".to_owned();
        values[27] = "ignored".to_owned();
        values[28] = "345".to_owned();
        values[29] = "1700000029".to_owned();
        for idx in 30..45 {
            values[idx] = (idx - 29).to_string();
        }
        values[45..].fill("ignored".to_owned());
        values
    }

    fn character_status_values() -> Vec<String> {
        let mut values = vec!["0".to_owned(); 33];
        values[0] = "ignored".to_owned();
        values[1] = "1".to_owned();
        values[2] = "8".to_owned();
        values[3] = "1700000003".to_owned();
        values[4] = "ignored".to_owned();
        values[5] = "12".to_owned();
        values[6] = "345".to_owned();
        values[7] = "4".to_owned();
        values[8] = "17".to_owned();
        values[9] = "1700000009".to_owned();
        values[10..20].fill("ignored".to_owned());
        values[20] = "1700000020".to_owned();
        values[21] = "1700000021".to_owned();
        values[22] = "1700000022".to_owned();
        values[23..].fill("ignored".to_owned());
        values
    }

    #[test]
    fn updates_the_whole_tower_response() {
        use crate::gamestate::underworld::{
            UnderworldBuildingType, UnderworldResourceType, UnderworldUnitType,
        };

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
        assert_eq!(data.len(), underworld::TOWER_RESPONSE_LEN);

        let mut state = GameState::default();
        state.update_tower(&data, ServerTime::default());
        data.assert_all_read();

        let companions = state.dungeons.companions.as_ref().unwrap();
        assert_eq!(companions[CompanionClass::Warrior].level, 360);
        assert_eq!(
            companions[CompanionClass::Warrior].attributes
                [AttributeType::Strength],
            4147
        );
        let uw = state.underworld.as_ref().unwrap();
        assert_eq!(
            uw.buildings[UnderworldBuildingType::HeartOfDarkness].level,
            15
        );
        assert_eq!(uw.buildings[UnderworldBuildingType::GoldPit].level, 29);
        assert_eq!(uw.units[UnderworldUnitType::Goblin].upgraded_amount, 416);
        assert_eq!(uw.units[UnderworldUnitType::Goblin].count, 5);
        assert_eq!(uw.souls_limit, 162_518_400);
        assert_eq!(
            uw.production[UnderworldResourceType::Souls].per_hour,
            49_500
        );
        assert_eq!(uw.lure_level, 504);
    }

    #[test]
    fn truncated_companion_equipment_uses_empty_defaults() {
        let raw = ["0"; EQUIPMENT_PARSE_LEN].join("/");
        let response = Response::parse(
            format!("companionequipment:{raw}"),
            NaiveDateTime::default(),
        )
        .unwrap();
        assert!(GameState::default().update(response).is_ok());
    }

    #[test]
    fn two_companion_equipment_blocks_keep_legacy_behavior() {
        let raw = ["0"; EQUIPMENT_PARSE_LEN * 2].join("/");
        let response = Response::parse(
            format!("companionequipment:{raw}"),
            NaiveDateTime::default(),
        )
        .unwrap();
        assert!(GameState::default().update(response).is_ok());
    }

    #[test]
    fn updates_the_whole_character_save_response() {
        let server_time = ServerTime(3_600);
        let raw = character_save_values().join("/");
        let data = Vals::from(raw.as_str());
        assert_eq!(data.len(), 70);

        let mut state = GameState::default();
        state.update_character_save(&data, server_time);
        data.assert_all_read();

        assert_eq!(state.character.player_id, 123_456);
        assert_eq!(state.character.level, 42);
        assert_eq!(state.character.experience, 9_000_000_000);
        assert_eq!(state.character.next_level_xp, 12_345_678);
        assert_eq!(state.character.honor, 4_321);
        assert_eq!(state.character.rank, 17);
        assert_eq!(state.character.race, Race::Dwarf);
        assert_eq!(state.character.class, Class::Assassin);
        assert_eq!(state.character.mount, Some(Mount::Dragon));
        assert_eq!(state.character.armor, 98_765);
        assert_eq!(state.character.min_damage, 123);
        assert_eq!(state.character.max_damage, 456);
        let expected_mount_end = server_time
            .convert_to_local(1_700_000_029, "mount end")
            .unwrap();
        assert_eq!(
            state.character.mount_end.as_ref(),
            Some(&expected_mount_end)
        );
        assert_eq!(
            state.character.portrait,
            Portrait {
                gender: Gender::Male,
                hair_color: 2,
                hair: 6,
                mouth: 4,
                brows: 3,
                eyes: 2,
                beards: 4,
                nose: 5,
                ears: 6,
                extra: 7,
                horns: 8,
                special_portrait: 9,
            }
        );
        assert_eq!(
            state.character.attribute_basis,
            EnumMap::from_array([1, 2, 3, 4, 5])
        );
        assert_eq!(
            state.character.attribute_additions,
            EnumMap::from_array([6, 7, 8, 9, 10])
        );
        assert_eq!(
            state.character.attribute_times_bought,
            EnumMap::from_array([11, 12, 13, 14, 15])
        );
        assert_eq!(state.guild.as_ref().unwrap().portal.damage_bonus, 12);
        assert_eq!(
            state.dungeons.portal.as_ref().unwrap().player_hp_bonus,
            345
        );
    }

    #[test]
    fn malformed_character_save_values_use_independent_defaults() {
        let server_time = ServerTime(3_600);
        let mut values = character_save_values();
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut state = GameState::default();
        state.update_character_save(&data, server_time);
        data.assert_all_read();

        for pos in [
            1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
            21, 23, 24, 25, 26,
        ] {
            values[pos] = "malformed".to_owned();
        }
        values[30..45].fill("malformed".to_owned());
        values[44] = "55".to_owned();

        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        state.update_character_save(&data, server_time);
        data.assert_all_read();

        assert_eq!(state.character.player_id, 0);
        assert_eq!(state.character.level, 0);
        assert_eq!(state.character.experience, 0);
        assert_eq!(state.character.next_level_xp, 0);
        assert_eq!(state.character.honor, 0);
        assert_eq!(state.character.rank, 0);
        assert_eq!(state.character.race, Race::Human);
        assert_eq!(state.character.class, Class::Warrior);
        assert_eq!(state.character.mount, None);
        assert_eq!(state.character.armor, 0);
        assert_eq!(state.character.min_damage, 0);
        assert_eq!(state.character.max_damage, 0);
        let expected_mount_end = server_time
            .convert_to_local(1_700_000_029, "mount end")
            .unwrap();
        assert_eq!(
            state.character.mount_end.as_ref(),
            Some(&expected_mount_end)
        );
        assert_eq!(
            state.character.portrait,
            Portrait {
                gender: Gender::Female,
                hair_color: 1,
                hair: 0,
                mouth: 1,
                brows: 1,
                eyes: 1,
                beards: 1,
                nose: 1,
                ears: 1,
                extra: 1,
                horns: 1,
                special_portrait: 0,
            }
        );
        assert_eq!(
            state.character.attribute_basis,
            EnumMap::from_array([0; 5])
        );
        assert_eq!(
            state.character.attribute_additions,
            EnumMap::from_array([0; 5])
        );
        assert_eq!(
            state.character.attribute_times_bought,
            EnumMap::from_array([0, 0, 0, 0, 55])
        );
        assert_eq!(state.guild.as_ref().unwrap().portal.damage_bonus, 0);
        assert_eq!(
            state.dungeons.portal.as_ref().unwrap().player_hp_bonus,
            345
        );
    }

    #[test]
    fn updates_the_whole_character_status_response() {
        let server_time = ServerTime(3_600);
        let raw = character_status_values().join("/");
        let data = Vals::from(raw.as_str());
        assert_eq!(data.len(), 33);

        let mut state = GameState::default();
        state.update_character_status(&data, server_time);
        data.assert_all_read();

        let expected_action_end = server_time
            .convert_to_local(1_700_000_003, "current action time")
            .unwrap();
        match state.tavern.current_action {
            CurrentAction::CityGuard { hours, busy_until } => {
                assert_eq!(hours, 8);
                assert_eq!(busy_until, expected_action_end);
            }
            action => panic!("unexpected current action: {action:?}"),
        }
        assert_eq!(state.tavern.beer_max, 12);
        assert_eq!(state.tavern.thirst_for_adventure_sec, 345);
        assert_eq!(state.tavern.beer_drunk, 4);
        assert_eq!(state.specials.calendar.collected, 17);
        let expected_calendar_next = server_time
            .convert_to_local(1_700_000_009, "calendar next")
            .unwrap();
        assert_eq!(
            state.specials.calendar.next_possible.as_ref(),
            Some(&expected_calendar_next)
        );
        let expected_pet_next = server_time
            .convert_to_local(1_700_000_020, "pet next free exploration")
            .unwrap();
        assert_eq!(
            state.pets.as_ref().unwrap().next_free_exploration.as_ref(),
            Some(&expected_pet_next)
        );
        let expected_dungeon_next = server_time
            .convert_to_local(1_700_000_021, "dungeon next free fight")
            .unwrap();
        assert_eq!(
            state.dungeons.next_free_fight.as_ref(),
            Some(&expected_dungeon_next)
        );
        let expected_healing_start = server_time
            .convert_to_local(1_700_000_022, "healing start")
            .unwrap();
        assert_eq!(
            state
                .legendary_dungeon
                .active
                .as_ref()
                .unwrap()
                .healing_start
                .as_ref(),
            Some(&expected_healing_start)
        );
    }

    #[test]
    fn zero_character_status_healing_time_clears_only_existing_state() {
        let server_time = ServerTime(3_600);
        let mut values = character_status_values();
        values[22] = "0".to_owned();
        let raw = values.join("/");
        let old_start = server_time
            .convert_to_local(1_700_000_100, "test healing start")
            .unwrap();

        let data = Vals::from(raw.as_str());
        let mut cached_state = GameState::default();
        cached_state.legendary_dungeon.active =
            Some(LegendaryDungeon::default());
        cached_state
            .legendary_dungeon
            .active
            .as_mut()
            .unwrap()
            .healing_start = Some(old_start);
        cached_state.update_character_status(&data, server_time);
        data.assert_all_read();
        assert!(
            cached_state
                .legendary_dungeon
                .active
                .as_ref()
                .unwrap()
                .healing_start
                .is_none()
        );

        let data = Vals::from(raw.as_str());
        let mut empty_state = GameState::default();
        empty_state.update_character_status(&data, server_time);
        data.assert_all_read();
        assert!(empty_state.legendary_dungeon.active.is_none());
    }

    #[test]
    fn truncated_malformed_character_status_uses_defaults() {
        let server_time = ServerTime(3_600);
        let raw = "ignored/malformed/malformed/malformed/ignored/malformed/\
                   malformed/malformed/malformed/malformed";
        let data = Vals::from(raw);
        let old_time = server_time
            .convert_to_local(1_700_000_100, "test status time")
            .unwrap();
        let mut state = GameState::default();
        state.tavern.current_action = CurrentAction::CityGuard {
            hours: 9,
            busy_until: old_time,
        };
        state.tavern.beer_max = 9;
        state.tavern.thirst_for_adventure_sec = 9;
        state.tavern.beer_drunk = 9;
        state.specials.calendar.collected = 9;
        state.specials.calendar.next_possible = Some(old_time);
        state.pets = Some(Pets {
            next_free_exploration: Some(old_time),
            ..Default::default()
        });
        state.dungeons.next_free_fight = Some(old_time);
        state.legendary_dungeon.active = Some(LegendaryDungeon::default());
        state
            .legendary_dungeon
            .active
            .as_mut()
            .unwrap()
            .healing_start = Some(old_time);

        state.update_character_status(&data, server_time);
        data.assert_all_read();

        assert_eq!(state.tavern.current_action, CurrentAction::Idle);
        assert_eq!(state.tavern.beer_max, 0);
        assert_eq!(state.tavern.thirst_for_adventure_sec, 0);
        assert_eq!(state.tavern.beer_drunk, 0);
        assert_eq!(state.specials.calendar.collected, 245);
        assert!(state.specials.calendar.next_possible.is_none());
        assert!(state.pets.as_ref().unwrap().next_free_exploration.is_none());
        assert!(state.dungeons.next_free_fight.is_none());
        assert!(
            state
                .legendary_dungeon
                .active
                .as_ref()
                .unwrap()
                .healing_start
                .is_none()
        );
    }

    #[test]
    fn portrait_tolerates_invalid_values_and_ignores_extras() {
        let data = Vals::from(
            "256/9223372036854775807/-1/-1/-1/-1/-1/-1/-1/99/-1/-1/extra",
        );
        let portrait = Portrait::parse_vals(&data);
        data.assert_all_read();

        assert_eq!(
            portrait,
            Portrait {
                gender: Gender::Female,
                hair_color: 100,
                hair: 7,
                mouth: 1,
                brows: 1,
                eyes: 1,
                beards: 1,
                nose: 1,
                ears: 1,
                extra: 1,
                horns: 1,
                special_portrait: 99,
            }
        );
    }
}
