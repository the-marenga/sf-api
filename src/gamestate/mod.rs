pub mod arena;
pub mod character;
pub mod dungeons;
pub mod event;
pub mod fortress;
pub mod guild;
pub mod hellevator;
pub mod idle;
pub mod items;
pub mod legendary_dungeon;
pub mod pets;
pub mod rewards;
pub mod social;
pub mod tavern;
pub mod underworld;
pub mod unlockables;

use std::{borrow::Borrow, collections::HashMap};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use enum_map::EnumMap;
use log::{error, warn};
use num_traits::FromPrimitive;
use strum::{EnumCount, IntoEnumIterator};

pub use crate::error::{Result, SFError};
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
        data: &[i64],
        server_time: ServerTime,
        typ: ShopType,
    ) -> Result<Shop, SFError> {
        let mut shop = Shop::default();
        shop.typ = typ;
        for (idx, item) in shop.items.iter_mut().enumerate() {
            let d = data.skip(idx * ITEM_PARSE_LEN, "shop item")?;
            let Some(p_item) = Item::parse_slice(d, server_time)? else {
                return Err(SFError::ParsingError(
                    "shop item",
                    format!("{d:?}"),
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

    pub(crate) fn update_resources(
        &mut self,
        res: &[i64],
    ) -> Result<(), SFError> {
        self.character.mushrooms = res.csiget(1, "mushrooms", 0)?;
        self.character.silver = res.csiget(2, "player silver", 0)?;
        self.tavern.quicksand_glasses =
            res.csiget(4, "quicksand glass count", 0)?;

        self.specials.wheel.lucky_coins = res.csiget(3, "lucky coins", 0)?;
        let bs = self.blacksmith.get_or_insert_default();
        bs.metal = res.csiget(9, "bs metal", 0)?;
        bs.arcane = res.csiget(10, "bs arcane", 0)?;
        let fortress = self.fortress.get_or_insert_default();
        fortress
            .resources
            .get_mut(FortressResourceType::Wood)
            .current = res.csiget(5, "saved wood ", 0)?;
        fortress
            .resources
            .get_mut(FortressResourceType::Stone)
            .current = res.csiget(7, "saved stone", 0)?;

        let pets = self.pets.get_or_insert_default();
        for (e_pos, element) in HabitatType::iter().enumerate() {
            pets.habitats.get_mut(element).fruits =
                res.csiget(12 + e_pos, "fruits", 0)?;
        }

        self.underworld.get_or_insert_default().souls_current =
            res.csiget(11, "uu souls saved", 0)?;
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
            companions
                .get_mut(class)
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
        let fights = &mut self.last_fight.get_or_insert_default().fights;

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
            "owngroupname" => {
                self.guild.get_or_insert_default().name.set(val.as_str());
            }
            "sfhomeid" => {}
            "backpack" => {
                self.character.inventory.backpack = val
                    .list()
                    .chunks(ITEM_PARSE_LEN)
                    .map(|chunk| Item::parse(&chunk, server_time))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "itemlevelshop" => {
                self.shop_item_lvl = val.into("shop lvl")?;
            }
            "storeitemsshakes" => {
                let data: Vec<i64> = val.into_list("weapon store")?;
                *self.shops.get_mut(ShopType::Weapon) =
                    Shop::parse(&data, server_time, ShopType::Weapon)?;
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
                if val.as_str().trim().is_empty() {
                    return Ok(());
                }
                let companions =
                    self.dungeons.companions.get_or_insert_default();
                let chunks = val.list();
                for (chunk, cmp) in chunks
                    .chunks(EQUIPMENT_PARSE_LEN)
                    .zip(companions.values_mut())
                {
                    cmp.equipment = Equipment::parse(&chunk, server_time)?;
                }
            }
            "storeitemsfidget" => {
                let data: Vec<i64> = val.into_list("magic store")?;
                *self.shops.get_mut(ShopType::Magic) =
                    Shop::parse(&data, server_time, ShopType::Magic)?;
            }
            "ownplayersaveequipment" => {
                let data = val.values::<EQUIPMENT_PARSE_LEN>()?;
                self.character.equipment =
                    Equipment::parse(&data, server_time)?;
            }
            "systemmessagelist" => {}
            "newslist" => {
                self.mail.update_news_inbox(&val.list_by(';'), server_time);
            }
            "dummieequipment" => {
                let data = val.values::<EQUIPMENT_PARSE_LEN>()?;
                self.character.mannequin =
                    Some(Equipment::parse(&data, server_time)?);
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
                    ChatMessage::parse_messages(val.as_str());
            }
            "chatwhisper" => {
                self.guild.get_or_insert_default().whispers =
                    ChatMessage::parse_messages(val.as_str());
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
                    uw.lure_suggestion =
                        val.as_str().parse::<u32>().ok().map(LureSuggestion);
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
                self.pending_unlocks = Unlockable::parse(&val.list())?;
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
                    .get_or_insert_default()
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
                self.update_gttime(&val.into_list("gttime")?, server_time)?;
            }
            "gtsave" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .update(&val.values::<32>()?, server_time)?;
            }
            "eventstatus" => {
                let data: Vec<i64> = val.into_list("event status")?;
                self.special_event = EventStatus::parse(&data, server_time)
                    .inspect_err(|a| {
                        log::warn!("Could not parse event status: {a}");
                    })
                    .ok();
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
                // Replaced
            }
            "owngroupdescription" => self
                .guild
                .get_or_insert_default()
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
                let data = val.values::<EQUIPMENT_PARSE_LEN>()?;
                other_player.get_or_insert_default().equipment =
                    Equipment::parse(&data, server_time)?;
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
                .get_or_insert_default()
                .update_building_prices(&val.values::<30>()?)?,
            "owngroupknights" => self
                .guild
                .get_or_insert_default()
                .update_group_knights(&val.list_by(',')),
            "friendlist" => self.updatete_relation_list(val.as_str()),
            "legendaries" => {
                if val.as_str().chars().any(|a| a != 'A') {
                    warn!("Found a legendaries value, that is not just AAA..");
                }
            }
            "smith" => {
                let data: Vec<i64> = val.into_list("smith")?;
                let bs = self.blacksmith.get_or_insert_default();

                bs.dismantle_left = data.csiget(0, "dismantles left", 0)?;
                bs.last_dismantled = data.cstget(1, "bs time", server_time)?;
            }
            "fortressGroupPrice" => {
                self.fortress
                    .get_or_insert_default()
                    .hall_of_knights_upgrade_price =
                    FortressCost::parse(&val.values::<4>()?)?;
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
                    ExpeditionBoss::parse(&val.values::<2>()?);
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
                self.character.scrapbook = ScrapBook::parse(&val.list());
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
                        &val.into_list("other potions")?,
                        server_time,
                    );
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
                other_player.get_or_insert_default().relationship =
                    warning_parse(
                        val.into::<i32>("other friend")?,
                        "other friend",
                        FromPrimitive::from_i32,
                    )
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
                let item = Item::parse(&data, server_time)?;
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
                let all: Vec<_> = val.as_str().split('/').collect();
                let hellevator = self.hellevator.active.get_or_insert_default();

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

                let hellevator = self.hellevator.active.get_or_insert_default();

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
                self.hellevator.active.get_or_insert_default().guild_rank =
                    val.into("gt rank").unwrap_or(0);
            }
            "gtrankingmax" => {
                self.hall_of_fames.hellevator_total =
                    val.into("gt rank max").ok();
            }
            "gtbracketlist" => {
                self.hellevator.active.get_or_insert_default().brackets =
                    val.into_list("gtbracketlist").unwrap_or_default();
            }
            "gtraidfights" => {
                let data: Vec<i64> =
                    val.into_list("gt raids").unwrap_or_default();

                let hellevator = self.hellevator.active.get_or_insert_default();

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

                let hellevator = self.hellevator.active.get_or_insert_default();
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
                    HellevatorMonster::parse(&val.list()).ok();
            }
            "gtbonus" => {
                self.hellevator
                    .active
                    .get_or_insert_default()
                    .daily_treat_bonus =
                    HellevatorTreatBonus::parse(&val.list()).ok();
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
                let data: Vec<i64> = val.into_list("wheel")?;
                // [0] => 2 ??
                self.specials.wheel.spins_today =
                    data.csiget(1, "lucky turns", 0)?;
                self.specials.wheel.next_free_spin =
                    data.cstget(2, "next lucky turn", server_time)?;
            }
            "dice" => {
                self.tavern
                    .dice_game
                    .update(&val.values::<2>()?, server_time);
            }
            "charactergroup" => {
                let data: Vec<i64> = val.into_list("c group")?;
                let guild = self.guild.get_or_insert_default();
                guild.own_treasure_skill =
                    data.csiget(0, "own treasure skill", 0)?;
                guild.own_instructor_skill =
                    data.csiget(1, "own instruction skill", 0)?;
                guild.hydra.next_battle =
                    data.cstget(2, "pet battle", server_time)?;
                guild.hydra.remaining_fights =
                    data.csiget(3, "remaining pet battles", 0)?;
                guild.own_pet_lvl = data.csiget(4, "own pet lvl", 0)?;
                guild.joined = data.cstget(5, "guild joined", server_time)?;
                // [6] => ????
            }
            "arena" => self.arena.update(&val.values::<7>()?, server_time)?,
            "ownplayersavepotions" => {
                let data: Vec<i64> = val.into_list("potions")?;
                self.character.active_potions =
                    items::parse_active_potions(&data, server_time);
            }
            "arcanetoilet" => {
                let data = val.values::<4>()?;

                // Toilet remains none as long as its level is 0
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
                let data: Vec<i64> = val.into_list("char status")?;

                self.tavern.current_action = CurrentAction::parse(
                    data.cget(1, "action id")?,
                    data.cget(2, "action sec")?,
                    data.cstget(3, "current action time", server_time)?,
                );

                // NOTE: [4] contains the start
                self.tavern.beer_max = data.csiget(5, "beer total", 0)?;

                self.tavern.thirst_for_adventure_sec =
                    data.csiget(6, "remaining ALU", 0)?;
                self.tavern.beer_drunk =
                    data.csiget(7, "beer drunk count", 0)?;
                self.specials.calendar.collected =
                    data.csiget(8, "calendar collected", 245)?;
                self.specials.calendar.next_possible =
                    data.cstget(9, "calendar next", server_time)?;
                // 0
                // 0
                // 0
                // 0
                // 0
                // 1513       // was [15]
                // 1541087831 // acc creation time?
                // 0
                // 0
                // 0
                self.pets.get_or_insert_default().next_free_exploration =
                    data.cstget(20, "pet next free exp", server_time)?;
                self.dungeons.next_free_fight =
                    data.cstget(21, "dungeon timer", server_time)?;
                if let Some(start) =
                    data.cstget(22, "dungeon timer", server_time)?
                {
                    self.legendary_dungeon
                        .active
                        .get_or_insert_default()
                        .healing_start = Some(start);
                }
                // 0
                // 1
                // 0
                // 0
                // 0
                // 0
                // 0
                // 0
                // 0
            }
            "ownplayersavecharacter" => {
                let data: Vec<i64> = val.into_list("char save")?;

                // 1482984989 // creation time? secret id?
                self.character.player_id = data.csiget(1, "player id", 0)?;
                // 0
                self.character.level =
                    data.csimget(3, "level", 0, |a| a & 0xFFFF)?;
                self.character.experience = data.csiget(4, "experience", 0)?;
                self.character.next_level_xp =
                    data.csiget(5, "xp to next lvl", 0)?;
                self.character.honor = data.csiget(6, "honor", 0)?;
                self.character.rank = data.csiget(7, "rank", 0)?;
                self.character.portrait =
                    Portrait::parse_slice(data.skip(8, "portrait")?)
                        .unwrap_or_default();
                ///// Portrait
                // 4
                // 206
                // 203
                // 2
                // 0
                // 2
                // 7
                // 2
                // 0
                // 0
                self.character.race = data.cfpuget(18, "char race", |a| a)?;
                // 2
                // ////
                self.character.class =
                    data.cfpuget(20, "character class", |a| a - 1)?;
                self.character.mount =
                    data.cfpget(21, "character mount", |a| a & 0xFF)?;
                // 0
                self.character.armor = data.csiget(23, "total armor", 0)?;
                self.character.min_damage = data.csiget(24, "min damage", 0)?;
                self.character.max_damage = data.csiget(25, "max damage", 0)?;
                self.guild.get_or_insert_default().portal.damage_bonus =
                    data.cimget(26, "portal dmg bonus", |a| a)?;
                // 4280492 ??
                self.dungeons.portal.get_or_insert_default().player_hp_bonus =
                    data.csimget(28, "portal hp bonus", 0, |a| a)?;
                self.character.mount_end =
                    data.cstget(29, "mount end", server_time)?;
                update_enum_map(
                    &mut self.character.attribute_basis,
                    data.skip(30, "char attr basis")?,
                );
                update_enum_map(
                    &mut self.character.attribute_additions,
                    data.skip(35, "char attr adds")?,
                );
                update_enum_map(
                    &mut self.character.attribute_times_bought,
                    data.skip(40, "char attr tb")?,
                );
                // 0
                // 0
                // 0
                // 0
                // 1
                // 17
                // 0
                // 0
                // 0
                // 66
                // 0
                // 7
                // 18
                // 0
                // 0
                // 0
                // 0
                // 0
                // 0
                // 0
                // guild_id ?
                // 12122    // sb count
                // 0
                // 31
                // 15       // gladiators
            }
            "adventure" => {
                self.tavern.update_quests(&val.values::<23>()?)?;
            }
            "events" => {
                let data: Vec<i64> = val.into_list("events")?;
                if data.len() < 8 {
                    return Ok(());
                }
                // [0] might be the events theme (tavern decoration)?
                self.specials.events.active.clear();
                let flags = data.cget(1, "events")?;
                for (idx, event) in Event::iter().enumerate() {
                    if (flags & (1 << idx)) > 0 {
                        self.specials.events.active.insert(event);
                    }
                }
                // NOTE: there are two end times, that seem to be identical.
                // I will just guess, that [4] is the correct one and [2] is
                // for the theme
                self.specials.events.ends =
                    data.cstget(4, "event end", server_time)?;

                // [3],[5],[6],[7] are just 0, so no idea what they could be
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
                let dungeons = &mut self.legendary_dungeon;

                let vals: Vec<i64> = val.into_list("iadungeontime")?;
                dungeons.theme = vals.cfpget(0, "ld theme", |x| x)?;
                dungeons.start = vals.cstget(1, "ld start", server_time)?;
                dungeons.end = vals.cstget(2, "ld end", server_time)?;
                dungeons.close = vals.cstget(3, "ld closes", server_time)?;
            }
            "iadungeonstatstotal" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();

                let data: Vec<i64> = val.into_list("iadungeonstatstotal")?;
                dungeons.total_stats = TotalStats::parse(&data)?;
            }
            "iadungeonstats" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();

                let data = val.into_list("iadungeonstats")?;
                dungeons.stats = Stats::parse(&data).unwrap_or_default();
            }
            "iadungeon" => {
                let data: Vec<i64> = val.into_list("iadungeon")?;
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.update(&data)?;
                if !all_values.contains_key("iapendingitems") {
                    dungeons.pending_items.clear();
                }
            }
            "iapendingitems" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();
                dungeons.pending_items.clear();
                let data: Vec<i64> = val.into_list("iapendingitems")?;
                let amount: i64 = data.cget(0, "pending amount")?;
                if amount < 1 {
                    return Ok(());
                }
                for slice in
                    data.skip(1, "ld items")?.chunks_exact(ITEM_PARSE_LEN)
                {
                    let Some(item) = Item::parse_slice(slice, server_time)?
                    else {
                        warn!("Could not parse pending ld item");
                        continue;
                    };
                    dungeons.pending_items.push(item);
                }
            }
            "ialootitem" => {
                // The stuff, that was looted after a fight
            }
            "iamerchant" => {
                let data: Vec<i64> = val.into_list("iamerchant")?;

                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .merchant_offers = data
                    .chunks_exact(3)
                    .flat_map(MerchantOffer::parse)
                    .flatten()
                    .collect();
            }
            "iadungeon20cost" => {
                self.legendary_dungeon
                    .active
                    .get_or_insert_default()
                    .heal_quarter_cost = val.into("iadungeon20cost")?;
            }
            "iadungeonsoulstones" => {
                let dungeons =
                    self.legendary_dungeon.active.get_or_insert_default();

                let data: Vec<i64> = val.into_list("iamerchant")?;
                let mut chunks = data.chunks_exact(6);
                dungeons.active_gems = chunks
                    .by_ref()
                    .take(3)
                    .flat_map(GemOfFate::parse)
                    .flatten()
                    .collect();

                dungeons.available_gems =
                    chunks.flat_map(GemOfFate::parse).flatten().collect();
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
    #![allow(clippy::unwrap_used)]
    use super::*;

    /// Asserts that every value of `data` was read or explicitly ignored.
    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

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
        assert_all_read(&data);

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
}
