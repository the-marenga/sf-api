pub mod arena;
pub mod character;
pub mod dungeons;
pub mod fortress;
pub mod guild;
pub mod idle;
pub mod items;
pub mod rewards;
pub mod social;
pub mod tavern;
pub mod underworld;
pub mod unlockables;

use std::{borrow::Borrow, collections::HashSet, i64};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use enum_map::EnumMap;
use log::{error, warn};
use num_traits::FromPrimitive;
use strum::IntoEnumIterator;

use self::underworld::Underworld;
use crate::{
    command::*,
    error::*,
    gamestate::{
        arena::*, character::*, dungeons::*, fortress::*, guild::*, idle::*,
        items::*, rewards::*, social::*, tavern::*, unlockables::*,
    },
    misc::*,
    session::*,
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
        });

        Self { items }
    }
}

impl Shop {
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Shop, SFError> {
        let mut shop = Shop::default();
        for (idx, item) in shop.items.iter_mut().enumerate() {
            let d = data.skip(idx * 12, "shop item")?;
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
    /// Constructs a new `GameState` from the provided response. The reponse has
    /// to be the login response from a `Session`.
    ///
    /// # Errors
    /// If the reponse contains any errors, or does not contain enough
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
                "preregister" | "languagecodelist" | "tracking"
                | "skipvideo" | "webshopid" | "cidstring" | "mountexpired" => {
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
                "toilettfull" => {
                    self.tavern
                        .toilet
                        .get_or_insert_with(Default::default)
                        .used = val.into::<i32>("toilet full status")? != 0;
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
                "fortresschest" => {
                    self.character.inventory.update_fortress_chest(
                        &val.into_list("fortress chest")?,
                        server_time,
                    )?;
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
                        companions.get_mut(class).equipment = Equipment::parse(
                            data.skip(comp_start + 22, "comp equip")?,
                            server_time,
                        )?;
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
                "owngroupsave" => {
                    self.guild
                        .get_or_insert_with(Default::default)
                        .update_group_save(
                            &val.into_list("guild save")?,
                            server_time,
                        )?;
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
                    self.dungeons
                        .portal
                        .get_or_insert_with(Default::default)
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
                "shadowlevel" => {
                    self.dungeons.update_levels(
                        &val.into_list("shadow dungeon levels")?,
                        DungeonType::Shadow,
                    );
                }
                "dungeonlevel" => {
                    self.dungeons.update_levels(
                        &val.into_list("shadow dungeon levels")?,
                        DungeonType::Light,
                    );
                }
                "gttime" => {
                    self.update_gttime(&val.into_list("gttime")?, server_time)?;
                }
                "gtsave" => {
                    self.hellevator.active = Hellevator::parse(
                        &val.into_list("gtsave")?,
                        server_time,
                    )?;
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
                    // I think they removed this
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
                    // time"); Pretty sure this is the time  something last
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
                    for (chunk, chest) in val
                        .into_list("daily task reward preview")?
                        .chunks_exact(5)
                        .zip(&mut self.specials.tasks.daily.rewards)
                    {
                        *chest = RewardChest::parse(chunk)?;
                    }
                }
                "expeditionevent" => {
                    let data: Vec<i64> = val.into_list("exp event")?;
                    self.tavern.expeditions.start =
                        data.cstget(0, "expedition start", server_time)?;
                    let end = data.cstget(1, "expedition end", server_time)?;
                    let end2 =
                        data.cstget(1, "expedition end2", server_time)?;
                    if end != end2 {
                        warn!("Weird expedition time");
                    }
                    self.tavern.expeditions.end = end;
                }
                "expeditions" => {
                    let data: Vec<i64> = val.into_list("exp event")?;

                    if data.len() % 8 != 0 {
                        warn!(
                            "Available expeditions have weird size: {data:?} \
                             {}",
                            data.len()
                        );
                    };
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
                    // for chunk in data.chunks_exact(12){
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
                        let task = EventTask::parse(c)?;
                        self.specials.tasks.event.tasks.push(task);
                    }
                }
                "eventtaskrewardpreview" => {
                    for (chunk, chest) in val
                        .into_list("event task reward preview")?
                        .chunks_exact(5)
                        .zip(&mut self.specials.tasks.event.rewards)
                    {
                        *chest = RewardChest::parse(chunk)?;
                    }
                }
                "dailytasklist" => {
                    let data: Vec<i64> = val.into_list("daily tasks list")?;
                    self.specials.tasks.daily.tasks.clear();

                    // I think the first value here is the amount of > 1 bell
                    // quests
                    for d in data.skip(1, "daily tasks")?.chunks_exact(4) {
                        self.specials
                            .tasks
                            .daily
                            .tasks
                            .push(DailyTask::parse(d)?);
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
                    self.character.scrapbok = ScrapBook::parse(val.as_str());
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
                        };
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
                    let Ok(mut op) = OtherPlayer::parse(
                        &val.into_list("other player")?,
                        server_time,
                    ) else {
                        // Should we err here?
                        other_player = None;
                        continue;
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
                "iadungeontime" => {
                    // No idea what this is measuring. Seems to just be a few
                    // days in the past, or just 0s.
                    // 1/1695394800/1696359600/1696446000
                }
                "workreward" => {
                    // Should be irrelevant
                }
                x if x.starts_with("winnerid") => {
                    self.get_fight(x).winner_id = val.into("winner id")?;
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
                    self.last_fight
                        .get_or_insert_with(Default::default)
                        .fight_version = val.into("fight version")?;
                }
                x if x.starts_with("fight") && x.len() <= 7 => {
                    self.get_fight(x).update_rounds(val.as_str())?;
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
                    let data: Vec<i64> = val.into_list("other group")?;
                    other_guild
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time)?;
                }
                "dummies" => {
                    self.character.manequin = Some(Equipment::parse(
                        &val.into_list("manequin")?,
                        server_time,
                    )?);
                }
                "reward" => {
                    // This is the task reward, which you should already know
                    // from collecting
                }
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
        if let Some(t) = &self.dungeons.portal {
            if t.finished == 0 {
                self.dungeons.portal = None;
            }
        }
        if let Some(pets) = &self.pets {
            if pets.rank == 0 {
                self.pets = None;
            }
        }
        if let Some(t) = &self.guild {
            if t.name.is_empty() {
                self.guild = None;
            }
        }
        if let Some(t) = &self.fortress {
            if t.upgrades == 0 {
                self.fortress = None;
            }
        }
        if let Some(t) = &self.underworld {
            if t.honor == 0 {
                self.underworld = None;
            }
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
        self.character.equipment =
            Equipment::parse(data.skip(48, "TODO")?, server_time)?;

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

        for (idx, item) in self.character.inventory.bag.iter_mut().enumerate() {
            let item_start = data.skip(168 + idx * 12, "inventory item")?;
            *item = Item::parse(item_start, server_time)?;
        }

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

        *self.shops.get_mut(ShopType::Weapon) =
            Shop::parse(data.skip(288, "TODO")?, server_time)?;
        *self.shops.get_mut(ShopType::Magic) =
            Shop::parse(data.skip(361, "TODO")?, server_time)?;

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

        self.character.druid_mask = data.cfpget(653, "druid mask", |a| a)?;
        self.character.bard_instrument =
            data.cfpget(701, "bard instrument", |a| a)?;
        self.specials.calendar.collected =
            data.csimget(648, "calendat collected", 245, |a| a >> 16)?;
        self.specials.calendar.next_possible =
            data.cstget(649, "calendar next", server_time)?;
        self.tavern.dice_game.next_free =
            data.cstget(650, "dice next", server_time)?;
        self.tavern.dice_game.remaining =
            data.csiget(651, "rem dice games", 0)?;

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
        let number_str =
            header_name.trim_start_matches(|a: char| !a.is_ascii_digit());
        let id: usize = number_str.parse().unwrap_or(1);
        let id = id.max(1);

        let fights =
            &mut self.last_fight.get_or_insert_with(Default::default).fights;

        if fights.len() < id {
            fights.resize(id, SingleFight::default());
        }
        #[allow(clippy::unwrap_used)]
        fights.get_mut(id - 1).unwrap()
    }
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
        if timestamp == 0 || timestamp == -1 || timestamp == 11 {
            // For some reason potions have 11 in the timestamp. No idea why
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
