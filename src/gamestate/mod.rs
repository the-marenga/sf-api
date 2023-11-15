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

use std::{array::from_fn, i64, mem::MaybeUninit};

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use log::warn;
use num_traits::FromPrimitive;
use strum::IntoEnumIterator;

use crate::{
    command::*,
    error::*,
    gamestate::{
        arena::*, character::*, dungeons::*, fortress::*, guild::*, idle::*,
        items::*, rewards::*, social::*, tavern::*, underworld::*,
        unlockables::*,
    },
    misc::*,
    session::*,
};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameState {
    pub character: CharacterState,

    /// Information about quests and work
    pub tavern: Tavern,
    pub arena: Arena,
    /// The last fight, that this player was involved in
    pub last_fight: Option<Fight>,
    // These are only ever none if an item in there was unable to be read,
    // which should almost never be the case. Oh, and it makes defaulting this
    // easy
    pub weapon_shop: Option<Shop>,
    pub mage_shop: Option<Shop>,
    /// Everything, that is time sensitive, like events, calendar, etc.
    pub special: Special,
    /// Everything, that the player needs
    pub unlocks: Unlockables,
    //  pub idle_game: Option<IdleGame>,
    pub other_players: OtherPlayers,
    /// The raw timestamp, that the server has send us
    last_request_timestamp: i64,
    /// The amount of sec, that the server is ahead of us in seconds (can we
    /// negative)
    server_time_diff: i64,
}

const SHOP_N: usize = 6;
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shop([Item; SHOP_N]);

impl Shop {
    pub(crate) fn parse(data: &[i64], server_time: ServerTime) -> Option<Shop> {
        // NOTE: I have no idea how to do this safely without multiple map()
        // calls, or a Vec to store them, as you can not return from within the
        // closures used to construct arrays
        let mut res = from_fn(|_| MaybeUninit::uninit());
        for (idx, uitem) in res.iter_mut().enumerate() {
            let item = Item::parse(&data[idx * 12..], server_time)?;
            *uitem = MaybeUninit::new(item);
        }
        // SAFETY: res is guaranteed to be init, as we iterate all items in the
        // uninit array and return on error. The input & outputs are strongly
        // typed, so we never transmute the wrong thing here in case Item should
        // ever return the wrong thing, or shop changes
        Some(Shop(unsafe {
            std::mem::transmute::<[MaybeUninit<Item>; SHOP_N], [Item; SHOP_N]>(
                res,
            )
        }))
    }
}

impl GameState {
    pub fn new(response: Response) -> Result<Self, SFError> {
        let mut res = Self::default();
        res.update(response)?;
        if res.character.level == 0 || res.character.name.is_empty() {
            return Err(SFError::ParsingError(
                "response did not contain full player state",
                "".to_string(),
            ));
        }
        Ok(res)
    }

    /// Updates the players information with the new data received from the
    /// server. Any error that is encounters terminates the update process
    pub fn update(&mut self, response: Response) -> Result<(), SFError> {
        use SFError::*;

        let new_vals = response.values();
        // Because the conversion of all other timestamps relies on the servers
        // timestamp, this has to be set first
        if let Some(ts) = new_vals.get("timestamp").copied() {
            let ts = ts.into("server time stamp")?;
            let server_time = NaiveDateTime::from_timestamp_opt(ts, 0)
                .ok_or(ParsingError("server time stamp", ts.to_string()))?;
            self.server_time_diff =
                (server_time - response.received_at()).num_seconds();
            self.last_request_timestamp = ts;
        }
        let server_time = self.server_time();

        self.last_fight = None;

        let mut other_player: Option<OtherPlayer> = None;
        let mut other_guild: Option<OtherGuild> = None;

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
                "gtchest" | "gtrank" | "gtbonus" | "gtbracketlist"
                | "gtrankingmax" => {
                    // Some hellevator stuff. TODO: Look at these next event
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
                        .used = val.into::<i32>("toilet full status")? != 0
                }
                "skipallow" => {
                    self.tavern.skip_allowed =
                        val.into::<i32>("skip allow")? != 0;
                }
                "cryptoid not found" => return Err(ConnectionError),
                "ownplayersave" => {
                    self.update_player_save(&val.into_list("player save")?)
                }
                "owngroupname" => self
                    .unlocks
                    .guild
                    .get_or_insert_with(Default::default)
                    .name
                    .set(val.as_str()),
                "tavernspecialsub" => {
                    self.special.events.clear();
                    let flags = val.into::<i32>("tavern special sub")?;
                    for (idx, event) in Event::iter().enumerate() {
                        if (flags & (1 << idx)) > 0 {
                            self.special.events.insert(event);
                        }
                    }
                }
                "fortresschest" => {
                    self.character.inventory.update_fortress_chest(
                        &val.into_list("fortress chest")?,
                        server_time,
                    );
                }
                "owntower" => {
                    let data = val.into_list("tower")?;
                    self.unlocks
                        .companions
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time);

                    // Why would they include this in the tower response???
                    self.unlocks
                        .underworld
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time);
                }
                "owngrouprank" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .rank = val.into("group rank")?;
                }
                "owngroupattack" | "owngroupdefense" => {
                    // Annoying
                }
                "owngroupsave" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .update_group_save(
                            &val.into_list("guild save")?,
                            server_time,
                        );
                }
                "owngroupmember" => self
                    .unlocks
                    .guild
                    .get_or_insert_with(Default::default)
                    .update_member_names(val.as_str()),
                "owngrouppotion" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .update_member_potions(val.as_str());
                }
                "unitprice" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .update_unit_prices(&val.into_list("fortress units")?);
                }
                "dicestatus" => {
                    let dices: Option<Vec<DiceType>> = val
                        .into_list("dice status")?
                        .into_iter()
                        .map(FromPrimitive::from_u8)
                        .collect();
                    self.tavern.current_dice = dices.unwrap_or_default();
                }
                "dicereward" => {
                    let data: Vec<u32> = val.into_list("dice reward")?;
                    let win_typ: DiceType = FromPrimitive::from_u32(
                        data[0] - 1,
                    )
                    .ok_or_else(|| {
                        SFError::ParsingError("dice reward", val.to_string())
                    })?;
                    self.tavern.dice_reward = Some(DiceReward {
                        win_typ,
                        amount: data[1],
                    })
                }
                "chathistory" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .chat = ChatMessage::parse_messages(val.as_str());
                }
                "chatwhisper" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .whispers = ChatMessage::parse_messages(val.as_str());
                }
                "upgradeprice" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .update_unit_upgrade_info(
                            &val.into_list("fortress unit upgrade prices")?,
                        );
                }
                "unitlevel" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .update_levels(&val.into_list("fortress unit levels")?);
                }
                "fortressprice" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .update_prices(
                            &val.into_list("fortress upgrade prices")?,
                        );
                }
                "witch" => {
                    self.unlocks
                        .witch
                        .get_or_insert_with(Default::default)
                        .update(&val.into_list("witch")?, server_time);
                }
                "underworldupgradeprice" => {
                    self.unlocks
                        .underworld
                        .get_or_insert_with(Default::default)
                        .update_underworld_unit_prices(
                            &val.into_list("underworld upgrade prices")?,
                        );
                }
                "unlockfeature" => {
                    self.unlocks.pending_unlocks =
                        Unlockable::parse(&val.into_list("unlock")?);
                }
                "dungeonprogresslight" => self.unlocks.dungeons.update(
                    &val.into_list("dungeon progress light")?,
                    DungeonType::Light,
                ),
                "dungeonprogressshadow" => self.unlocks.dungeons.update(
                    &val.into_list("dungeon progress shadow")?,
                    DungeonType::Shadow,
                ),
                "portalprogress" => {
                    self.unlocks
                        .portal
                        .get_or_insert_with(Default::default)
                        .update(&val.into_list("portal progress")?);
                }
                "tavernspecialend" => {
                    self.special.events_ends = server_time
                        .convert_to_local(val.into("event end")?, "event end");
                }
                "owntowerlevel" => {
                    // Already in dungeons
                }
                "serverversion" => {
                    // Handled in session
                }
                "stoneperhournextlevel" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .quarry_next_level_production =
                        val.into("stone next lvl")?;
                }
                "woodperhournextlevel" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .woodcutter_next_level_production =
                        val.into("wood next lvl")?;
                }
                "shadowlevel" => {
                    self.unlocks.dungeons.update_levels(
                        &val.into_list("shadow dungeon levels")?,
                        DungeonType::Shadow,
                    );
                }
                "dungeonlevel" => {
                    self.unlocks.dungeons.update_levels(
                        &val.into_list("shadow dungeon levels")?,
                        DungeonType::Light,
                    );
                }
                "gttime" => {
                    self.update_gttime(&val.into_list("gttime")?, server_time);
                }
                "gtsave" => {
                    self.update_gtsave(&val.into_list("gtsave")?, server_time);
                }
                "maxrank" => {
                    self.other_players.total_player =
                        val.into("player count")?;
                }
                "achievement" => {
                    self.unlocks
                        .achievements
                        .update(&val.into_list("achievements")?);
                }
                "groupskillprice" => {
                    self.unlocks
                        .guild
                        .get_or_insert_with(Default::default)
                        .update_group_prices(
                            &val.into_list("guild skill prices")?,
                        );
                }
                "soldieradvice" => {
                    // I think they removed this
                }
                "owngroupdescription" => self
                    .unlocks
                    .guild
                    .get_or_insert_with(Default::default)
                    .update_description_embed(val.as_str()),
                "idle" => {
                    self.unlocks.idle_game = IdleGame::parse_idle_game(
                        val.into_list("idle game")?,
                        server_time,
                    );
                }
                "resources" => {
                    self.update_resources(&val.into_list("resources")?);
                }
                "chattime" => {
                    // let _chat_time = server_time
                    //     .convert_to_local(val.into("chat time")?, "chat
                    // time"); Pretty sure this is the time  something last
                    // happened in chat, but nobody cares and messages have a
                    // time
                }
                "maxpetlevel" => {
                    self.unlocks
                        .pet_collection
                        .get_or_insert_with(Default::default)
                        .max_pet_level = val.into("max pet lvl")?;
                }
                "otherdescription" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .description = from_sf_string(val.as_str());
                }
                "otherplayergroupname" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .guild_name
                        .set(val.as_str());
                }
                "otherplayername" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .name
                        .set(val.as_str());
                }
                "fortresspricereroll" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .opponent_reroll_price = val.into("fortress reroll")?;
                }
                "fortresswalllevel" => {
                    self.unlocks
                        .fortress
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
                        && self.unlocks.pet_collection.is_some()
                        && self.unlocks.underworld.is_some();
                    self.tavern.wheel_result = Some(WheelReward::parse(
                        &val.into_list("wheel result")?,
                        upgraded,
                    )?);
                }
                "dailyreward" => {
                    // Dead since last update
                }
                "calenderreward" => {
                    // Probably removed and shoould be irrelevant
                }
                "calenderinfo" => {
                    // This is twice in the original response.
                    // This API sucks LMAO
                    let data: Vec<i64> = val.into_list("calendar")?;
                    self.special.calendar.clear();
                    for p in data.chunks_exact(2) {
                        if let Some(reward) = CalendarReward::parse(p) {
                            self.special.calendar.push(reward)
                        } else {
                            warn!("Could not parse calendar value: {p:?}");
                            break;
                        }
                    }
                }
                "othergroupattack" => {
                    other_guild.get_or_insert_with(Default::default).attacks =
                        Some(val.to_string())
                }
                "othergroupdefense" => {
                    other_guild
                        .get_or_insert_with(Default::default)
                        .defends_against = Some(val.to_string())
                }
                "inboxcapacity" => {
                    self.other_players.inbox_capacity =
                        val.into("inbox cap")?;
                }
                "magicregistration" => {
                    // Pretty sure this means you have not provided a pw or
                    // mail. Just a name and clicked play
                }
                "Ranklistplayer" => {
                    self.other_players.hall_of_fame.clear();
                    for player in val.as_str().trim_matches(';').split(';') {
                        let data: Vec<_> = player.split(',').collect();
                        if data.len() < 6 {
                            warn!("Invalid hof player: {:?}", data);
                            continue;
                        }
                        let (Some(rank), Some(level), Some(fame), Some(class)) = (
                            warning_from_str(data[0], "invalid hof rank"),
                            warning_from_str(data[3], "invalid hof level"),
                            warning_from_str(data[4], "invalid hof fame"),
                            warning_from_str::<i64>(
                                data[5],
                                "invalid hof class",
                            ),
                        ) else {
                            continue;
                        };
                        let Some(class) = FromPrimitive::from_i64(class - 1)
                        else {
                            warn!("Invalid hof class: {class} - {:?}", data);
                            continue;
                        };
                        self.other_players.hall_of_fame.push(HallOfFameEntry {
                            rank,
                            name: data[1].to_string(),
                            guild: data[2].to_string(),
                            level,
                            fame,
                            class,
                            flag: data
                                .get(6)
                                .copied()
                                .unwrap_or_default()
                                .to_string(),
                        });
                    }
                }
                "ranklistgroup" => {
                    self.other_players.guild_hall_of_fame.clear();
                    for guild in val.as_str().trim_matches(';').split(';') {
                        let data: Vec<_> = guild.split(',').collect();
                        if data.len() != 6 {
                            warn!("Invalid hof guild: {:?}", data);
                            continue;
                        }
                        let (
                            Some(rank),
                            Some(member),
                            Some(honor),
                            Some(attack_status),
                        ) = (
                            warning_from_str(data[0], "invalid hof rank"),
                            warning_from_str(data[3], "invalid hof level"),
                            warning_from_str(data[4], "invalid hof fame"),
                            warning_from_str::<u8>(data[5], "invalid hof atk"),
                        )
                        else {
                            continue;
                        };
                        self.other_players.guild_hall_of_fame.push(
                            HallOfFameGuildEntry {
                                rank,
                                name: data[1].to_string(),
                                leader: data[2].to_string(),
                                member,
                                honor,
                                is_attacked: attack_status == 1,
                            },
                        );
                    }
                }
                "maxrankgroup" => {
                    self.other_players.total_guilds =
                        Some(val.into("guild max")?)
                }
                "maxrankPets" => {
                    self.other_players.total_pet_players =
                        Some(val.into("pet rank max")?)
                }
                "RanklistPets" => {
                    self.other_players.pets_hall_of_fame.clear();
                    for entry in val.as_str().trim_matches(';').split(';') {
                        let data: Vec<_> = entry.split(',').collect();
                        if data.len() != 6 {
                            warn!("Invalid hof guild: {:?}", data);
                            continue;
                        }
                        let (
                            Some(rank),
                            Some(collected),
                            Some(honor),
                            Some(unknown),
                        ) = (
                            warning_from_str(data[0], "invalid hof rank"),
                            warning_from_str(data[3], "invalid hof level"),
                            warning_from_str(data[4], "invalid hof fame"),
                            warning_from_str(data[5], "invalid hof atk"),
                        )
                        else {
                            continue;
                        };
                        self.other_players.pets_hall_of_fame.push(
                            HallOfFamePetsEntry {
                                rank,
                                name: data[1].to_string(),
                                guild: data[2].to_string(),
                                collected,
                                honor,
                                unknown,
                            },
                        );
                    }
                }
                "ranklistfortress" | "Ranklistfortress" => {
                    self.other_players.fortress_hall_of_fame.clear();
                    for guild in val.as_str().trim_matches(';').split(';') {
                        let data: Vec<_> = guild.split(',').collect();
                        if data.len() != 6 {
                            warn!("Invalid hof guild: {:?}", data);
                            continue;
                        }
                        let (
                            Some(rank),
                            Some(upgrade),
                            Some(honor),
                            Some(unknown),
                        ) = (
                            warning_from_str(data[0], "invalid hof rank"),
                            warning_from_str(data[3], "invalid hof level"),
                            warning_from_str(data[4], "invalid hof fame"),
                            warning_from_str(data[5], "invalid hof atk"),
                        )
                        else {
                            continue;
                        };
                        self.other_players.fortress_hall_of_fame.push(
                            HallOfFameFortressEntry {
                                rank,
                                name: data[1].to_string(),
                                guild: data[2].to_string(),
                                upgrade,
                                honor,
                                unknown,
                            },
                        );
                    }
                }
                "ranklistunderworld" => {
                    self.other_players.underworld_hall_of_fame.clear();
                    for entry in val.as_str().trim_matches(';').split(';') {
                        let data: Vec<_> = entry.split(',').collect();
                        if data.len() != 6 {
                            warn!("Invalid hof underworld: {:?}", data);
                            continue;
                        }
                        let (
                            Some(rank),
                            Some(upgrade),
                            Some(honor),
                            Some(unknown),
                        ) = (
                            warning_from_str(data[0], "invalid hof rank"),
                            warning_from_str(data[3], "invalid hof level"),
                            warning_from_str(data[4], "invalid hof fame"),
                            warning_from_str(data[5], "invalid hof atk"),
                        )
                        else {
                            continue;
                        };
                        self.other_players.underworld_hall_of_fame.push(
                            HallOfFameUnderworldEntry {
                                rank,
                                name: data[1].to_string(),
                                guild: data[2].to_string(),
                                upgrade,
                                honor,
                                unknown,
                            },
                        );
                    }
                }
                "gamblegoldvalue" => {
                    self.special.gamble_result = Some(
                        GambleResult::SilverChange(val.into("gold gamble")?),
                    );
                }
                "gamblecoinvalue" => {
                    self.special.gamble_result = Some(
                        GambleResult::MushroomChange(val.into("gold gamble")?),
                    );
                }
                "maxrankFortress" => {
                    self.other_players.total_fortresses =
                        Some(val.into("fortress max")?)
                }
                "underworldprice" => self
                    .unlocks
                    .underworld
                    .get_or_insert_with(Default::default)
                    .update_building_prices(&val.into_list("ub prices")?),
                "owngroupknights" => self
                    .unlocks
                    .guild
                    .get_or_insert_with(Default::default)
                    .update_group_knights(val.as_str()),
                "friendlist" => {
                    self.other_players.updatete_relation_list(val.as_str())
                }
                "legendaries" => {
                    if val.as_str().chars().any(|a| a != 'A') {
                        warn!(
                            "Found a legendaries value, that is not just AAA.."
                        )
                    }
                }
                "smith" => {
                    let data: Vec<i64> = val.into_list("smith")?;
                    let bs = self
                        .unlocks
                        .blacksmith
                        .get_or_insert_with(Default::default);

                    bs.dismantle_left =
                        soft_into(data[0], "dismantles left", 0);
                    bs.last_dismantled =
                        server_time.convert_to_local(data[1], "bs time");
                }
                "tavernspecial" => {
                    // Pretty sure this has been replaced
                }
                "fortressGroupPrice" => {
                    // No idea what this is: "0/0/21880000/7200000"
                }
                "goldperhournextlevel" => {
                    // I dont think this matters
                }
                "underworldmaxsouls" => {
                    // This should already be in resources
                }
                "dailytaskrewardpreview" => {
                    for (chunk, chest) in val
                        .into_list("event task reward preview")?
                        .chunks_exact(5)
                        .zip(&mut self.special.daily_quest_rewards)
                    {
                        *chest = RewardChest::parse(chunk)
                    }
                }
                "expeditionevent" => {
                    let data = val.into_list("exp event")?;
                    self.tavern.expedition_start =
                        server_time.convert_to_local(data[0], "a");
                    let end = server_time.convert_to_local(data[1], "b");
                    let end2 = server_time.convert_to_local(data[1], "b");
                    if end != end2 {
                        warn!("Weird expedition time")
                    }
                    self.tavern.expedition_end = end;
                }
                "expeditions" => {
                    let data: Vec<i64> = val.into_list("exp event")?;

                    if data.len() != 16 {
                        warn!("Not two expedition? {data:?} {}", data.len());
                        self.tavern.expeditions = None;
                        continue;
                    };
                    self.tavern.expeditions = Some([0, 8].map(|a| {
                        ExpeditionInfo {
                            target: warning_parse(
                                data[a],
                                "expedition typ",
                                FromPrimitive::from_i64,
                            )
                            .unwrap_or_default(),
                            alu_sec: soft_into(data[6 + a], "exp alu", 600),
                            location1_id: data[4 + a],
                            location2_id: data[5 + a],
                        }
                    }));
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
                        .expedition
                        .get_or_insert_with(Default::default);

                    if data[0] == -100 {
                        exp.boss = None;
                        continue;
                    };
                    exp.boss = Some(ExpeditionBoss {
                        id: warning_parse(
                            -data[0],
                            "expedition monster",
                            FromPrimitive::from_i64,
                        )
                        .unwrap_or_default(),
                        items: soft_into(
                            data.get(1).copied().unwrap_or_default(),
                            "exp monster items",
                            3,
                        ),
                    });
                }
                "expeditionhalftime" => {
                    let data: Vec<i64> = val.into_list("halftime exp")?;
                    let exp = self
                        .tavern
                        .expedition
                        .get_or_insert_with(Default::default);
                    exp.halftime_choice =
                        data[1..].chunks_exact(2).map(Reward::parse).collect();
                }
                "expeditionstate" => {
                    let data: Vec<i64> = val.into_list("exp state")?;
                    let exp = self
                        .tavern
                        .expedition
                        .get_or_insert_with(Default::default);

                    exp.target = warning_parse(
                        data[3],
                        "expedition target",
                        FromPrimitive::from_i64,
                    )
                    .unwrap_or_default();
                    exp.current = soft_into(data[7], "exp current", 100);
                    exp.target_amount = soft_into(data[8], "exp target", 100);

                    exp.clearing = soft_into(data[0], "clearing", 0);
                    exp.heroism = soft_into(data[13], "clearing", 0);

                    let _busy_since =
                        server_time.convert_to_local(data[15], "exp start");

                    exp.busy_until =
                        server_time.convert_to_local(data[16], "exp busy");

                    exp.items = [0, 1, 2, 3].map(|i| {
                        let x = data[9 + i];
                        if x == 0 {
                            return None;
                        }
                        Some(match FromPrimitive::from_i64(x) {
                            Some(x) => x,
                            None => {
                                warn!("Unknown item: {x}");
                                ExpeditionThing::Unknown
                            }
                        })
                    });
                }
                "expeditioncrossroad" => {
                    // 3/3/132/0/2/2
                    let data: Vec<i64> = val.into_list("cross")?;
                    let exp = self
                        .tavern
                        .expedition
                        .get_or_insert_with(Default::default);
                    exp.crossroads = [0, 2, 4].map(|pos| {
                        let typ = match FromPrimitive::from_i64(data[pos]) {
                            Some(x) => x,
                            None => {
                                warn!(
                                    "Unknown crossroad enc: {} for {}",
                                    data[pos], pos
                                );
                                ExpeditionThing::Unknown
                            }
                        };
                        let heroism = soft_into(data[pos + 1], "e heroism", 0);
                        ExpeditionEncounter { typ, heroism }
                    });
                }
                "eventtasklist" => {
                    let data: Vec<i64> = val.into_list("etl")?;
                    self.special.event_tasks.clear();
                    for c in data.chunks_exact(4) {
                        match EventTask::parse(c) {
                            Some(x) => self.special.event_tasks.push(x),
                            None => {
                                warn!(
                                    "Could not parse {c:?} into an event task"
                                )
                            }
                        }
                    }
                }
                "eventtaskrewardpreview" => {
                    let data: Vec<i64> =
                        val.into_list("event task reward preview")?;

                    self.special.event_tasks_rewards[0] =
                        RewardChest::parse(&data[0..5]);
                    self.special.event_tasks_rewards[1] =
                        RewardChest::parse(&data[5..10]);
                    self.special.event_tasks_rewards[2] =
                        RewardChest::parse(&data[10..]);
                }
                "dailytasklist" => {
                    let data: Vec<i64> = val.into_list("daily tasks list")?;
                    self.special.daily_quests.clear();

                    // I think the first value here is the amount of > 1 bell
                    // quests
                    for d in data[1..].chunks_exact(4) {
                        match DailyQuest::parse(d) {
                            Some(d) => self.special.daily_quests.push(d),
                            None => {
                                warn!("Bad daily quest: {d:?}");
                                continue;
                            }
                        }
                    }
                }
                "eventtaskinfo" => {
                    let data: Vec<i64> = val.into_list("eti")?;
                    self.special.event_task_typ = warning_parse(
                        data[2],
                        "event task typ",
                        FromPrimitive::from_i64,
                    );
                    self.special.event_task_start =
                        server_time.convert_to_local(data[0], "event t start");
                    self.special.event_task_end =
                        server_time.convert_to_local(data[1], "event t end");
                }
                "scrapbook" => {
                    // I hate this
                }
                "dungeonfaces" | "shadowfaces" => {
                    // Gets returned after winning a dungeon fight. This looks a
                    // bit like a reward, but that should be handled in fight
                    // parsing already?
                }
                "messagelist" => {
                    let data = val.as_str();
                    self.other_players.inbox.clear();
                    for msg in data.split(';').filter(|a| !a.trim().is_empty())
                    {
                        if let Some(msg) = InboxEntry::parse(msg, server_time) {
                            self.other_players.inbox.push(msg)
                        };
                    }
                }
                "messagetext" => {
                    self.other_players.open_msg =
                        Some(from_sf_string(val.as_str()));
                }
                "combatloglist" => {
                    for entry in val.as_str().split(';') {
                        let parts = entry.split(',').collect::<Vec<_>>();
                        if let Some(cle) =
                            CombatLogEntry::parse(&parts, server_time)
                        {
                            self.other_players.combat_log.push(cle);
                        } else if parts.iter().all(|a| !a.is_empty()) {
                            warn!(
                                "Unable to parse combat log entry: {parts:?}"
                            );
                        }
                    }
                }
                "maxupgradelevel" => {
                    self.unlocks
                        .fortress
                        .get_or_insert_with(Default::default)
                        .building_max_lvl = val.into("max upgrade lvl")?
                }
                "singleportalenemylevel" => {
                    self.unlocks
                        .portal
                        .get_or_insert_with(Default::default)
                        .enemy_level = val.into("portal lvl")?;
                }
                "ownpetsstats" => {
                    self.unlocks
                        .pet_collection
                        .get_or_insert_with(Default::default)
                        .update_pet_stat(&val.into_list("pet stats")?);
                }
                "ownpets" => {
                    let data = val.into_list("own pets")?;
                    self.unlocks
                        .pet_collection
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time);
                }
                "petsdefensetype" => {
                    let pet_id = val.into("pet def typ")?;
                    self.unlocks
                        .pet_collection
                        .get_or_insert_with(Default::default)
                        .enemy_pet_type =
                        Some(PetClass::from_typ_id(pet_id).ok_or(
                            ParsingError("pet def typ", format!("{pet_id}")),
                        )?);
                }
                "otherplayer" => {
                    let Some(mut op) = OtherPlayer::parse(
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
                        op.guild_name = oop.guild_name;
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
                        .update_pet_bonus(&val.into_list("o pet bonus")?);
                }
                "otherplayerunitlevel" => {
                    let data: Vec<i64> =
                        val.into_list("other player unit level")?;
                    // This includes other levels, but they are handled
                    // elsewhere I think
                    other_player
                        .get_or_insert_with(Default::default)
                        .wall_combat_lvl = soft_into(data[0], "wall_lvl", 0);
                }
                "petsrank" => {
                    self.unlocks
                        .pet_collection
                        .get_or_insert_with(Default::default)
                        .rank = val.into("pet rank")?;
                }

                "maxrankUnderworld" => {
                    self.other_players.total_underworld_players =
                        Some(val.into("mrank under")?);
                }
                "otherplayerfortressrank" => {
                    other_player
                        .get_or_insert_with(Default::default)
                        .fortress_rank =
                        match val.into::<i64>("other friend fortress rank")? {
                            ..=-1 => None,
                            x => Some(x as u32),
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
                        .fight_version = val.into("fight version")?
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

                    guild.emblem.set(emblem);
                    guild.description = from_sf_string(desc);
                }
                "othergroup" => {
                    let data: Vec<i64> = val.into_list("other group")?;
                    other_guild
                        .get_or_insert_with(Default::default)
                        .update(&data, server_time);
                }
                "dummies" => {
                    self.character.manequin = Some(Equipment::parse(
                        &val.into_list("manequin")?,
                        server_time,
                    ));
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
                _x => {
                    warn!("Update ignored {_x} -> {val:?}");
                }
            }
        }

        if let Some(og) = other_guild {
            self.other_players.guilds.insert(og.name.clone(), og);
        }
        if let Some(other_player) = other_player {
            self.other_players.insert_lookup(other_player)
        }
        if let Some(t) = &self.unlocks.portal {
            if t.current == 0 {
                self.unlocks.portal = None;
            }
        }
        if let Some(pets) = &self.unlocks.pet_collection {
            if pets.rank == 0 {
                self.unlocks.pet_collection = None
            }
        }
        if let Some(t) = &self.unlocks.guild {
            if t.name.is_empty() {
                self.unlocks.guild = None;
            }
        }
        if let Some(t) = &self.unlocks.fortress {
            if t.level == 0 {
                self.unlocks.fortress = None;
            }
        }
        if let Some(t) = &self.unlocks.underworld {
            if t.total_level == 0 {
                self.unlocks.underworld = None;
            }
        }
        Ok(())
    }

    pub(crate) fn update_player_save(&mut self, data: &[i64]) {
        let server_time = self.server_time();
        if data.len() < 700 {
            warn!("Skipping account update");
            return;
        }

        self.character.player_id = soft_into(data[1], "player id", 0);
        self.character.portrait.update(&data[17..]);

        self.character.equipment = Equipment::parse(&data[48..], server_time);

        self.character.armor = soft_into(data[447], "total armor", 0);
        self.character.min_damage = soft_into(data[448], "min damage", 0);
        self.character.max_damage = soft_into(data[449], "max damage", 0);

        self.character.level = soft_into(data[7] & 0xFF, "level", 0);
        self.character.experience = soft_into(data[8], "experience", 0);
        self.character.next_level_xp = soft_into(data[9], "xp to next lvl", 0);
        self.character.honor = soft_into(data[10], "honor", 0);
        self.character.rank = soft_into(data[11], "rank", 0);
        self.character.class =
            FromPrimitive::from_i64((data[29] & 0xFF) - 1).unwrap_or_default();
        self.character.race =
            FromPrimitive::from_i64(data[27] & 0xFF).unwrap_or_default();

        self.tavern.update(data, server_time);

        self.character.attribute_basis.update(&data[30..]);
        self.character.attribute_additions.update(&data[35..]);
        self.character.attribute_times_bought.update(&data[40..]);

        self.character.mount = FromPrimitive::from_i64(data[286] & 0xFF);
        self.character.mount_end =
            server_time.convert_to_local(data[451], "mount end");

        for (idx, item) in self.character.inventory.bag.iter_mut().enumerate() {
            *item = Item::parse(&data[(168 + idx * 12)..], server_time);
        }

        if self.character.level >= 25 {
            let fortress =
                self.unlocks.fortress.get_or_insert_with(Default::default);
            fortress.update(data, server_time);
        }

        self.character.active_potions =
            ItemType::parse_active_potions(&data[493..], server_time);
        self.character.wheel_spins_today =
            soft_into(data[579], "lucky turns", 0);
        self.character.wheel_next_free_spin =
            warning_parse(data[580], "next lucky turn", |a| {
                server_time.convert_to_local(a, "next lucky turn")
            });

        self.weapon_shop = Shop::parse(&data[288..], server_time);
        self.mage_shop = Shop::parse(&data[361..], server_time);

        self.unlocks.mirror = Mirror::parse(data[28]);
        if data[438] >= 10000 {
            self.unlocks.scrapbook_count =
                Some(soft_into(data[438] - 10000, "scrapbook count", 0));
        }
        self.arena.next_free_fight =
            server_time.convert_to_local(data[460], "next battle time");
        self.arena.next_free_fight =
            server_time.convert_to_local(data[460], "next battle time");

        // Toilet remains none as long as its level is 0
        if data[491] > 0 {
            self.tavern
                .toilet
                .get_or_insert_with(Default::default)
                .update(data);
        }

        for (idx, val) in self.arena.enemy_ids.iter_mut().enumerate() {
            *val = soft_into(data[599 + idx], "enemy_id", 0)
        }

        if let Some(jg) =
            server_time.convert_to_local(data[443], "guild join date")
        {
            self.unlocks
                .guild
                .get_or_insert_with(Default::default)
                .joined = jg;
        }

        self.unlocks.dungeon_timer =
            server_time.convert_to_local(data[459], "dungeon timer");

        self.unlocks
            .pet_collection
            .get_or_insert_with(Default::default)
            .dungeon_timer =
            server_time.convert_to_local(data[660], "pet dungeon time");

        self.unlocks
            .portal
            .get_or_insert_with(Default::default)
            .player_hp_bonus =
            soft_into((data[445] >> 16) / 256, "portal hp bonus", 0);

        let guild = self.unlocks.guild.get_or_insert_with(Default::default);
        guild.guild_portal.damage_bonus = ((data[445] >> 16) % 256) as u8;
        guild.own_treasure_skill =
            soft_into(data[623], "own treasure skill", 0);
        guild.own_instruction_skill =
            soft_into(data[624], "own treasure skill", 0);
        guild.hydra_next_battle =
            server_time.convert_to_local(data[627], "pet battle");
        self.unlocks
            .pet_collection
            .get_or_insert_with(Default::default)
            .remaining_pet_battles =
            soft_into(data[628], "remaining pet battles", 0);
        self.character.druid_mask = FromPrimitive::from_i64(data[653]);
        self.character.bard_instrument = FromPrimitive::from_i64(data[701]);

        self.special.calendar_next_possible =
            server_time.convert_to_local(data[649], "calendar next");
        self.tavern.dice_games_next_free =
            server_time.convert_to_local(data[650], "dice next");
        self.tavern.dice_games_remaining =
            soft_into(data[651], "rem dice games", 0);
    }

    pub(crate) fn update_gttime(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) {
        let d = self.unlocks.hellevator.get_or_insert_with(Default::default);
        d.event_start = server_time.convert_to_local(data[0], "event start");
        d.event_end = server_time.convert_to_local(data[1], "event end");
        d.collect_time_end =
            server_time.convert_to_local(data[3], "claim time end");
    }

    pub(crate) fn update_resources(&mut self, res: &[i64]) {
        self.character.mushrooms = soft_into(res[1], "mushrooms", 0);
        self.character.silver = soft_into(res[2], "player silver", 0);
        self.tavern.quicksand_glasses =
            soft_into(res[4], "quicksand glass count", 0);

        self.character.lucky_coins = soft_into(res[3], "lucky coins", 0);
        let bs = self.unlocks.blacksmith.get_or_insert_with(Default::default);
        bs.metal = soft_into(res[9], "bs metal", 0);
        bs.arcane = soft_into(res[10], "bs arcane", 0);
        let fortress =
            self.unlocks.fortress.get_or_insert_with(Default::default);
        fortress.resources[FortressResourceType::Wood as usize].limit =
            soft_into(res[5], "saved wood ", 0);
        fortress.resources[FortressResourceType::Stone as usize].limit =
            soft_into(res[7], "saved stone", 0);

        let pets = self
            .unlocks
            .pet_collection
            .get_or_insert_with(Default::default);
        for i in 0..5 {
            pets.fruits[i] = soft_into(res[12 + i], "fruits", 0);
        }

        self.unlocks
            .underworld
            .get_or_insert_with(Default::default)
            .resources[UnderWorldResourceType::Souls as usize]
            .current = soft_into(res[11], "uu souls saved", 0);
    }

    pub(crate) fn update_gtsave(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) {
        let d = self.unlocks.hellevator.get_or_insert_with(Default::default);
        d.key_cards = soft_into(data[0], "h key cards", 0);
        d.next_card_generated =
            server_time.convert_to_local(data[1], "next card");
        d.next_reset = server_time.convert_to_local(data[2], "next reset");
        d.current_floor = soft_into(data[3], "h current floor", 0);
        d.points = soft_into(data[4], "h points", 0);
        d.start_contrib_date =
            server_time.convert_to_local(data[5], "start contrib");
        d.has_final_reward = data[6] == 1;
        d.points_today = soft_into(data[10], "h points today", 0);
    }

    /// Returns the time of the server. This is just an 8 byte copy behind the
    /// scenes, so feel free to NOT cache/optimize calling this in any way
    pub fn server_time(&self) -> ServerTime {
        ServerTime(self.server_time_diff)
    }

    /// Given a header value like "fight4", this would give you the
    /// corresponding fight[3]. In case that does not exist, it will be created
    /// w/ the default
    fn get_fight(&mut self, header_name: &str) -> &mut SingleFight {
        let id = header_name
            .chars()
            .position(|a| a.is_ascii_digit())
            .map(|a| &header_name[a..])
            .and_then(|a| a.parse::<usize>().ok())
            .unwrap_or(1);

        let fights =
            &mut self.last_fight.get_or_insert_with(Default::default).fights;

        if fights.len() < id {
            fights.resize(id, Default::default())
        }
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
    pub fn convert_to_local(
        &self,
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

        NaiveDateTime::from_timestamp_opt(timestamp - self.0, 0)?
            .and_local_timezone(Local)
            .latest()
    }

    /// The current time of the server in their time zone (whatever that might
    /// be). This uses the system time and calculates the offset to the
    /// servers time, so this is NOT the time at the last request, but the
    /// actual current time of the server.
    pub fn current(&self) -> NaiveDateTime {
        Local::now().naive_local() + Duration::seconds(self.0)
    }
}

// https://stackoverflow.com/a/59955929
trait StringSetExt {
    fn set(&mut self, s: &str);
}

impl StringSetExt for String {
    /// Replace the contents of a string with a string slice. This is basically
    /// self = s.to_string(), but without the deallication of self + allocation
    /// of s for that
    fn set(&mut self, s: &str) {
        self.replace_range(.., s);
    }
}
