pub mod arena;
pub mod character;
pub mod dungeons;
pub mod fortress;
pub mod guild;
pub mod idle;
pub mod items;
pub mod legendary_dungeons;
pub mod rewards;
pub mod social;
pub mod tavern;
pub mod underworld;
pub mod unlockables;

use std::{borrow::Borrow, collections::HashSet};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use enum_map::EnumMap;
use log::{error, warn};
use num_traits::FromPrimitive;
use strum::{EnumCount, IntoEnumIterator};

use crate::{
    command::*,
    error::*,
    gamestate::{
        arena::*,
        character::*,
        dungeons::*,
        fortress::*,
        guild::*,
        idle::*,
        items::*,
        legendary_dungeons::{
            DungeonEffect, DungeonStats, LegendaryDungeonsEvent, RoomEncounter,
        },
        rewards::*,
        social::*,
        tavern::*,
        underworld::*,
        unlockables::*,
    },
    misc::*,
    response::Response,
};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Represent the full state of the game at some point in time
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
    pub legendary_dungeons: LegendaryDungeonsEvent,
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
    /// The raw timestamp, that the server has send us
    last_request_timestamp: i64,
    /// The amount of sec, that the server is ahead of us in seconds (can be
    /// negative)
    server_time_diff: i64,
}

const SHOP_N: usize = 6;
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A shop, that you can buy items from
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
        });

        Self {
            items,
            typ: ShopType::Magic,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Copy)]
pub struct ShopPosition {
    pub(crate) typ: ShopType,
    pub(crate) pos: usize,
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
            let Some(p_item) = Item::parse(d, server_time)? else {
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

        #[allow(clippy::match_same_arms)]
        for (key, val) in new_vals.iter().map(|(a, b)| (*a, *b)) {
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
                    self.update_player_save(&val.into_list("player save")?)?;
                }
                "owngroupname" => self
                    .guild
                    .get_or_insert_with(Default::default)
                    .name
                    .set(val.as_str()),
                "tavernspecialsub" => {
                    self.specials.events.active.clear();
                    let flags = val.into::<i32>("tavern special sub")?;
                    for (idx, event) in Event::iter().enumerate() {
                        if (flags & (1 << idx)) > 0 {
                            self.specials.events.active.insert(event);
                        }
                    }
                }
                "sfhomeid" => {}
                "backpack" => {
                    let data: Vec<i64> = val.into_list("backpack")?;
                    self.character.inventory.backpack = data
                        .chunks_exact(ITEM_PARSE_LEN)
                        .map(|a| Item::parse(a, server_time))
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
                    for (chunk, quest) in val
                        .into_list("quest items")?
                        .chunks_exact(19)
                        .zip(&mut self.tavern.quests)
                    {
                        quest.item = Item::parse(chunk, server_time)?;
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
                        continue;
                    }
                    let toilet = self.tavern.toilet.get_or_insert_default();
                    toilet.sacrifices_left = vals[2] as u32;
                    toilet.used = toilet.sacrifices_left == 0;
                }
                "companionequipment" => {
                    let data: Vec<i64> = val.into_list("quest items")?;
                    for (idx, cmp) in self
                        .dungeons
                        .companions
                        .get_or_insert_with(Default::default)
                        .values_mut()
                        .enumerate()
                    {
                        let data = data.skip(
                            (19 * EquipmentSlot::COUNT) * idx,
                            "companion item",
                        )?;
                        cmp.equipment = Equipment::parse(data, server_time)?;
                    }
                }
                "storeitemsfidget" => {
                    let data: Vec<i64> = val.into_list("magic store")?;
                    *self.shops.get_mut(ShopType::Magic) =
                        Shop::parse(&data, server_time, ShopType::Magic)?;
                }
                "ownplayersaveequipment" => {
                    let data: Vec<i64> = val.into_list("player equipment")?;
                    self.character.equipment =
                        Equipment::parse(&data, server_time)?;
                }
                "systemmessagelist" => {}
                "newslist" => {}
                "dummieequipment" => {
                    let m: Vec<i64> = val.into_list("manequin")?;
                    self.character.manequin =
                        Some(Equipment::parse(&m, server_time)?);
                }
                "owntower" => {
                    let data = val.into_list("tower")?;
                    let companions = self
                        .dungeons
                        .companions
                        .get_or_insert_with(Default::default);

                    for (i, class) in CompanionClass::iter().enumerate() {
                        let comp_start = 3 + i * 148;
                        companions.get_mut(class).level =
                            data.cget(comp_start, "comp level")?;
                        update_enum_map(
                            &mut companions.get_mut(class).attributes,
                            data.skip(comp_start + 4, "comp attrs")?,
                        );
                    }
                    // Why would they include this in the tower response???
                    self.underworld
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time)?;
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
                        .update_group_save(val.as_str(), server_time)?;
                }
                "owngroupmember" => self
                    .guild
                    .get_or_insert_with(Default::default)
                    .update_member_names(val.as_str()),
                "owngrouppotion" => {
                    self.guild
                        .get_or_insert_with(Default::default)
                        .update_member_potions(val.as_str());
                }
                "unitprice" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .update_unit_prices(
                            &val.into_list("fortress units")?,
                        )?;
                }
                "dicestatus" => {
                    let dices: Option<Vec<DiceType>> = val
                        .into_list("dice status")?
                        .into_iter()
                        .map(FromPrimitive::from_u8)
                        .collect();
                    self.tavern.dice_game.current_dice =
                        dices.unwrap_or_default();
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
                        .update_unit_upgrade_info(
                            &val.into_list("fortress unit upgrade prices")?,
                        )?;
                }
                "unitlevel" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .update_levels(
                            &val.into_list("fortress unit levels")?,
                        )?;
                }
                "fortressprice" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .update_prices(
                            &val.into_list("fortress upgrade prices")?,
                        )?;
                }
                "witch" => {
                    self.witch
                        .get_or_insert_with(Default::default)
                        .update(&val.into_list("witch")?, server_time)?;
                }
                "underworldupgradeprice" => {
                    self.underworld
                        .get_or_insert_with(Default::default)
                        .update_underworld_unit_prices(
                            &val.into_list("underworld upgrade prices")?,
                        )?;
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
                    self.dungeons.portal.get_or_insert_with(Default::default)
                        .update(&val.into_list("portal progress")?, server_time)?;
                }
                "tavernspecialend" => {
                    self.specials.events.ends = server_time
                        .convert_to_local(val.into("event end")?, "event end");
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
                "maxrank" => {
                    self.hall_of_fames.players_total =
                        val.into("player count")?;
                }
                "achievement" => {
                    self.achievements
                        .update(&val.into_list("achievements")?)?;
                }
                "groupskillprice" => {
                    self.guild
                        .get_or_insert_with(Default::default)
                        .update_group_prices(
                            &val.into_list("guild skill prices")?,
                        )?;
                }
                "soldieradvice" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .soldier_advice =
                        val.into::<u16>("other player soldier advice").ok();
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
                    self.pets
                        .get_or_insert_with(Default::default)
                        .max_pet_level = val.into("max pet lvl")?;
                }
                "otherdescription" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .description = from_sf_string(val.as_str());
                }
                "otherplayergroupname" => {
                    let guild = Some(val.as_str().to_string())
                        .filter(|a| !a.is_empty());
                    other_player.get_or_insert_with(Default::default).guild =
                        guild;
                }
                "otherplayername" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .name
                        .set(val.as_str());
                }
                "otherplayersaveequipment" => {
                    let data: Vec<i64> =
                        val.into_list("other player equipment")?;
                    other_player
                        .get_or_insert_with(Default::default)
                        .equipment = Equipment::parse(&data, server_time)?;
                }
                "fortresspricereroll" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .opponent_reroll_price = val.into("fortress reroll")?;
                }
                "fortresswalllevel" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .wall_combat_lvl = val.into("fortress wall lvl")?;
                }
                "dragongoldbonus" => {
                    self.character.mount_dragon_refund =
                        val.into("dragon gold")?;
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
                    let vals: Vec<_> = val.as_str().split('/').collect();
                    let v = match vals.as_slice().cget(4, "questing setting")? {
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
                    self.hall_of_fames.guilds_total =
                        Some(val.into("guild max")?);
                }
                "maxrankPets" => {
                    self.hall_of_fames.pets_total =
                        Some(val.into("pet rank max")?);
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
                    self.tavern.gamble_result = Some(
                        GambleResult::SilverChange(val.into("gold gamble")?),
                    );
                }
                "gamblecoinvalue" => {
                    self.tavern.gamble_result = Some(
                        GambleResult::MushroomChange(val.into("gold gamble")?),
                    );
                }
                "maxrankFortress" => {
                    self.hall_of_fames.fortresses_total =
                        Some(val.into("fortress max")?);
                }
                "underworldprice" => self
                    .underworld
                    .get_or_insert_with(Default::default)
                    .update_building_prices(&val.into_list("ub prices")?)?,
                "owngroupknights" => self
                    .guild
                    .get_or_insert_with(Default::default)
                    .update_group_knights(val.as_str()),
                "friendlist" => self.updatete_relation_list(val.as_str()),
                "legendaries" => {
                    if val.as_str().chars().any(|a| a != 'A') {
                        warn!(
                            "Found a legendaries value, that is not just AAA.."
                        );
                    }
                }
                "smith" => {
                    let data: Vec<i64> = val.into_list("smith")?;
                    let bs =
                        self.blacksmith.get_or_insert_with(Default::default);

                    bs.dismantle_left = data.csiget(0, "dismantles left", 0)?;
                    bs.last_dismantled =
                        data.cstget(1, "bs time", server_time)?;
                }
                "tavernspecial" => {
                    // Pretty sure this has been replaced
                }
                "fortressGroupPrice" => {
                    self.fortress
                        .get_or_insert_with(Default::default)
                        .hall_of_knights_upgrade_price = FortressCost::parse(
                        &val.into_list("hall of knights prices")?,
                    )?;
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
                            "Available expeditions have weird size: {data:?} \
                             {}",
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
                                thirst_for_adventure_sec: data
                                    .csiget(6, "exp alu", 600)?,
                                location_1: data
                                    .cfpget(4, "exp loc 1", |a| a)?
                                    .unwrap_or_default(),
                                location_2: data
                                    .cfpget(5, "exp loc 2", |a| a)?
                                    .unwrap_or_default(),
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

                    let _busy_since =
                        data.cstget(15, "exp start", server_time)?;
                    exp.busy_until =
                        data.cstget(16, "exp busy", server_time)?;

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
                    for msg in data.split(';').filter(|a| !a.trim().is_empty())
                    {
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
                                    "Unable to parse combat log entry: \
                                     {parts:?} - {e}"
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
                        .enemy_level =
                        val.into("portal lvl").unwrap_or(u32::MAX);
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
                        .habitat =
                        Some(HabitatType::from_typ_id(pet_id).ok_or(
                            SFError::ParsingError(
                                "pet def typ",
                                format!("{pet_id}"),
                            ),
                        )?);
                }
                "otherplayer" => {
                    let mut op = match OtherPlayer::parse(
                        &val.into_list("other player")?,
                        server_time,
                    ) {
                        Ok(op) => op,
                        Err(e) => {
                            warn!("{e}");
                            // Should we err here?
                            other_player = None;
                            continue;
                        }
                    };

                    // TODO: This sucks! Change parse -> update
                    if let Some(oop) = other_player {
                        op.name = oop.name;
                        op.description = oop.description;
                        op.guild = oop.guild;
                        op.relationship = oop.relationship;
                        op.pet_attribute_bonus_perc =
                            oop.pet_attribute_bonus_perc;
                        op.wall_combat_lvl = oop.wall_combat_lvl;
                        op.fortress_rank = oop.fortress_rank;
                        op.soldier_advice = oop.soldier_advice;
                        op.equipment = oop.equipment;
                    }
                    other_player = Some(op);
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
                    other_player
                        .get_or_insert_with(Default::default)
                        .fortress_rank =
                        match val.into::<i64>("other player fortress rank")? {
                            ..=-1 => None,
                            x => Some(x.try_into().unwrap_or(1)),
                        };
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
                    let data: Vec<i64> = val.into_list("fight result")?;
                    self.last_fight
                        .get_or_insert_with(Default::default)
                        .update_result(&data, server_time)?;
                    // Note: The sub_key from this, can improve fighter parsing
                }
                x if x.starts_with("fightheader") => {
                    self.get_fight(x).update_fighters(val.as_str());
                }
                "fightgroups" => {
                    let fight =
                        self.last_fight.get_or_insert_with(Default::default);
                    fight.update_groups(val.as_str());
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
                    let fight_no = fight_no_from_header(x);
                    let wkey = format!("winnerid{fight_no}");
                    let version =
                        if let Some(winner_id) = new_vals.get(wkey.as_str()) {
                            // For unknown reasons, the fightversion is merged
                            // into the winnerid for all fights, except the last
                            // one
                            winner_id
                                .as_str()
                                .split_once("fightversion:")
                                .map(|a| a.1)
                        } else {
                            // The last fight uses the normal fightversion
                            // header
                            new_vals.get("fightversion").map(|a| a.as_str())
                        };
                    let fight = self.get_fight(x);
                    if let Some(version) = version.and_then(|a| a.parse().ok())
                    {
                        fight.update_rounds(val.as_str(), version)?;
                    } else {
                        fight.actions.clear();
                    }
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
                    other_guild
                        .get_or_insert_with(Default::default)
                        .attack_cost = val.into("other group fighting cost")?;
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
                    let guild =
                        other_guild.get_or_insert_with(Default::default);
                    let (emblem, desc) = val
                        .as_str()
                        .split_once('§')
                        .unwrap_or(("", val.as_str()));

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
                    let hellevator = self
                        .hellevator
                        .active
                        .get_or_insert_with(Default::default);

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

                    let hellevator = self
                        .hellevator
                        .active
                        .get_or_insert_with(Default::default);

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

                    let hellevator = self
                        .hellevator
                        .active
                        .get_or_insert_with(Default::default);

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
                    for (data, floor) in start
                        .chunks_exact(5)
                        .zip(&mut hellevator.guild_raid_floors)
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

                    let hellevator = self
                        .hellevator
                        .active
                        .get_or_insert_with(Default::default);
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
                        if !new_vals.contains_key("gtdailyreward") {
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
                    let vals: Vec<i64> = val.into_list("pending item")?;
                    self.mail
                        .open_claimable
                        .get_or_insert_with(Default::default)
                        .items = vals
                        .chunks_exact(ITEM_PARSE_LEN)
                        .flat_map(|a|
                            // Might be broken
                            Item::parse(a, server_time))
                        .flatten()
                        .collect();
                }
                "fightablegroups" => {
                    self.guild
                        .get_or_insert_default()
                        .update_fightable_targets(val.as_str())?;
                }
                // Legendary Dungeons
                "iadungeontime" => {
                    let dungeons = &mut self.legendary_dungeons;

                    let vals: Vec<i64> = val.into_list("iadungeontime")?;
                    dungeons.theme = vals.cfpget(0, "ld theme", |x| x)?;
                    dungeons.start_time =
                        vals.cstget(1, "ld start", server_time)?;
                    dungeons.end_time =
                        vals.cstget(2, "ld end", server_time)?;
                    dungeons.close_time =
                        vals.cstget(3, "ld closes", server_time)?;
                }
                "iadungeonstatstotal" => {
                    let _dungeons =
                        self.legendary_dungeons.active.get_or_insert_default();

                    let data: Vec<i64> =
                        val.into_list("iadungeonstatstotal")?;
                    log::info!("iadungeonstatstotal: {data:?}");
                    // [0] => ?
                    // [1] => ?
                    // [2] => total keys
                    // [3] => ?
                    // [4] => total gold
                    // [5] => ?
                }
                "iadungeonstats" => {
                    let dungeons =
                        self.legendary_dungeons.active.get_or_insert_default();

                    let data = val.into_list("iadungeonstats")?;
                    dungeons.stats =
                        DungeonStats::parse(&data).unwrap_or_default();
                }
                "iadungeon" => {
                    let data: Vec<i64> = val.into_list("iadungeon")?;
                    let dungeons =
                        self.legendary_dungeons.active.get_or_insert_default();

                    // [00] 718719374 <= Some sort of random id?
                    // [01] 2 <= ?

                    dungeons.current_hp = data.csiget(2, "ld current hp", 0)?;
                    dungeons.pre_battle_hp = data.csiget(3, "ld pre hp", 0)?;
                    dungeons.max_hp = data.csiget(4, "ld max hp", 0)?;

                    for (pos, v) in dungeons.blessings.iter_mut().enumerate() {
                        let s = data.csiget(11 + pos, "ld blessing rem", 0)?;
                        *v = DungeonEffect::parse(
                            data.csiget(5 + pos, "ld blessing typ", 0)?,
                            s / 10_000,
                            data.csiget(42 + pos, "ld blessing max", 0)?,
                            s % 10_000,
                        );
                    }
                    for (pos, v) in dungeons.curses.iter_mut().enumerate() {
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

                    dungeons.stage = data
                        .cfpuget(15, "dungeon stage", |a| a)
                        .unwrap_or_default();

                    // [16] 0 // ?

                    dungeons.current_floor = data.csiget(17, "ld floor", 0)?;
                    dungeons.max_floor = data.csiget(18, "ld max floor", 0)?;

                    for (pos, v) in dungeons.doors.iter_mut().enumerate() {
                        *v = data
                            .cfpuget(19 + pos, "dungeon stage", |a| a)
                            .unwrap_or_default();
                    }

                    // [21] 0 // ?

                    let raw_enc = data.csiget(22, "ld max floor", 999)?;
                    dungeons.encounter = RoomEncounter::parse(raw_enc);

                    // Most of this should be an unused (moved) item, but I
                    // don't know where it starts and ends

                    // [23] 0
                    // [24] 0
                    // [25] 0
                    // [26] 0
                    // [27] 0
                    // [28] 0
                    // [29] 0
                    // [30] 0
                    // [31] 0
                    // [32] 0
                    // [33] 0
                    // [34] 0
                    // [35] 0
                    // [36] 0
                    // [37] 0
                    // [38] 0

                    dungeons.keys = data.csiget(39, "ld keys", 0)?;

                    // [40] 0
                    // [41] 0

                    // [48] 0
                    // [49] 0

                    for (pos, n) in data.iter().enumerate() {
                        log::info!("[{pos}] {n}");
                    }
                }
                "iapendingitems" => {
                    let data: Vec<i64> = val.into_list("iapendingitems")?;

                    let item = Item::parse(&data[1..], server_time)?;
                    log::info!("Current item {}:  {item:?}", data[0]);
                }
                "iamerchant" => {
                    //    "0/0/0/0/0/0"
                    log::info!("iamerchant: {val}");

                    // 2/40100/0/3/50080/0

                    // [0] => type (2 => One Hit Wonder, 3 => Escape assistant)
                    // [1] => effect duration / strength
                    // [2] => price
                }
                "iadungeon20cost" => {
                    //    "10"
                    log::info!("iadungeon20cost: {val}");
                }
                "iadungeonsoulstones" => {
                    // 0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/
                    // 0/0/0/0/0/0/0/0"
                    log::info!("iadungeonsoulstones: {val}");
                }
                "iamap" => {
                    // Pre: 25/-5159/1/-315/25/-5160/1/-315/25/-5172/1/-315/25/
                    // -5168/1/-315" 1:   25/-5159/1/-315/
                    // 25/-5160/1/-315/25/-5172/1/-315/25/-5168/1/-315

                    // [0] => Amount of levels
                    // [1] => name of dungeon / effect?
                    // [2] => max key master shops?
                    // [3] => ?
                    // 25/-5159/1/-315/
                    // 25/-5160/1/-315/
                    // 25/-5172/1/-315/
                    // 25/-5168/1/-315

                    log::info!("iamap: {val}");
                }

                // This is the extra bonus effect all treats get that day
                x if x.contains("dungeonenemies") => {
                    // I `think` we do not need this
                }
                x if x.starts_with("attbonus") => {
                    // This is always 0s, so I have no idea what this could be
                }
                x => {
                    warn!("Update ignored {x} -> {val:?}");
                }
            }
        }

        if let Some(exp) = self.tavern.expeditions.active_mut() {
            exp.adjust_bounty_heroism();
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
        if let Some(t) = &self.underworld
            && t.buildings[UnderworldBuildingType::HeartOfDarkness].level < 1
        {
            self.underworld = None;
        }

        // Witch is automatically unlocked with level 66
        if self.witch.is_some() && self.character.level < 66 {
            self.witch = None;
        }

        Ok(())
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
    pub(crate) fn update_player_save(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        let server_time = self.server_time();
        if data.len() < 700 {
            warn!("Skipping account update");
            return Ok(());
        }

        self.character.player_id = data.csiget(1, "player id", 0)?;
        self.character.portrait =
            Portrait::parse(data.skip(17, "TODO")?).unwrap_or_default();

        self.character.armor = data.csiget(447, "total armor", 0)?;
        self.character.min_damage = data.csiget(448, "min damage", 0)?;
        self.character.max_damage = data.csiget(449, "max damage", 0)?;

        self.character.level = data.csimget(7, "level", 0, |a| a & 0xFFFF)?;
        self.arena.fights_for_xp =
            data.csimget(7, "arena xp fights", 0, |a| a >> 16)?;

        self.character.experience = data.csiget(8, "experience", 0)?;
        self.character.next_level_xp = data.csiget(9, "xp to next lvl", 0)?;
        self.character.honor = data.csiget(10, "honor", 0)?;
        self.character.rank = data.csiget(11, "rank", 0)?;
        self.character.class =
            data.cfpuget(29, "character class", |a| (a & 0xFF) - 1)?;
        self.character.race =
            data.cfpuget(27, "character race", |a| a & 0xFF)?;

        self.tavern.update(data, server_time)?;

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

        self.character.mount =
            data.cfpget(286, "character mount", |a| a & 0xFF)?;
        self.character.mount_end =
            data.cstget(451, "mount end", server_time)?;

        if self.character.level >= 25 {
            let fortress = self.fortress.get_or_insert_with(Default::default);
            fortress.update(data, server_time)?;
        }

        self.character.active_potions = ItemType::parse_active_potions(
            data.skip(493, "TODO")?,
            server_time,
        );
        self.specials.wheel.spins_today = data.csiget(579, "lucky turns", 0)?;
        self.specials.wheel.next_free_spin =
            data.cstget(580, "next lucky turn", server_time)?;

        self.character.mirror = Mirror::parse(data.cget(28, "mirror start")?);
        self.arena.next_free_fight =
            data.cstget(460, "next battle time", server_time)?;

        // Toilet remains none as long as its level is 0
        let toilet_lvl = data.cget(491, "toilet lvl")?;
        if toilet_lvl > 0 {
            self.tavern
                .toilet
                .get_or_insert_with(Default::default)
                .update(data)?;
        }

        for (idx, val) in self.arena.enemy_ids.iter_mut().enumerate() {
            *val = data.csiget(599 + idx, "enemy_id", 0)?;
        }

        if let Some(jg) = data.cstget(443, "guild join date", server_time)? {
            self.guild.get_or_insert_with(Default::default).joined = jg;
        }

        self.dungeons.next_free_fight =
            data.cstget(459, "dungeon timer", server_time)?;

        self.pets
            .get_or_insert_with(Default::default)
            .next_free_exploration =
            data.cstget(660, "pet next free exp", server_time)?;

        self.dungeons
            .portal
            .get_or_insert_with(Default::default)
            .player_hp_bonus =
            data.csimget(445, "portal hp bonus", 0, |a| a >> 24)?;

        let guild = self.guild.get_or_insert_with(Default::default);
        // TODO: This might be better as & 0xFF?
        guild.portal.damage_bonus =
            data.cimget(445, "portal dmg bonus", |a| (a >> 16) % 256)?;
        guild.own_treasure_skill = data.csiget(623, "own treasure skill", 0)?;
        guild.own_instructor_skill =
            data.csiget(624, "own instruction skill", 0)?;
        guild.hydra.next_battle =
            data.cstget(627, "pet battle", server_time)?;
        guild.hydra.remaining_fights =
            data.csiget(628, "remaining pet battles", 0)?;

        // self.character.druid_mask = data.cfpget(653, "druid mask", |a| a)?;
        // self.character.bard_instrument =
        //     data.cfpget(701, "bard instrument", |a| a)?;

        self.specials.calendar.collected =
            data.csimget(648, "calendar collected", 245, |a| a >> 16)?;
        self.specials.calendar.next_possible =
            data.cstget(649, "calendar next", server_time)?;
        self.tavern.dice_game.next_free =
            data.cstget(650, "dice next", server_time)?;
        self.tavern.dice_game.remaining =
            data.csiget(651, "rem dice games", 0)?;

        self.witch.get_or_insert_default().enchantment_price =
            data.csiget(519, "enchantment price", u64::MAX)?;

        Ok(())
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The cost of something
pub struct NormalCost {
    /// The amount of silver something costs
    pub silver: u64,
    /// The amount of mushrooms something costs
    pub mushrooms: u16,
}
