#![allow(deprecated)]
use enum_map::Enum;
use num_derive::FromPrimitive;

use crate::{
    gamestate::{
        character::*,
        dungeons::{LightDungeon, ShadowDungeons},
        fortress::*,
        guild::GuildSkill,
        idle::IdleBuildingType,
        items::*,
        social::Relationship,
        underworld::*,
        unlockables::Unlockable,
    },
    misc::{sha1_hash, to_sf_string, HASH_CONST},
    PlayerId,
};

// A command, that can be send to the sf server
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Command {
    /// If there is a command you somehow know/reverse engineered, or need to
    /// extend the functionality of one of the existing commands, this is the
    /// command for you
    Custom {
        cmd_name: String,
        values: Vec<String>,
    },
    /// Manually sends a login request to the server.
    /// **WARN:** The behaviour for a credentials mismatch, with the
    /// credentials in the user is undefined. Use the login method instead
    /// for a safer abstraction
    #[deprecated = "Use the login method instead"]
    Login {
        username: String,
        pw_hash: String,
        login_count: u32,
    },
    #[cfg(feature = "sso")]
    /// Manually sends a login request to the server.
    /// **WARN:** The behaviour for a credentials mismatch, with the
    /// credentials in the user is undefined. Use the login method instead for
    /// a safer abstraction
    #[deprecated = "Use a login method instead"]
    SSOLogin {
        uuid: String,
        character_id: String,
        bearer_token: String,
    },
    #[deprecated = "Use the register method instead"]
    Register {
        username: String,
        password: String,
        gender: Gender,
        race: Race,
        class: Class,
    },
    /// Updates the current state of the user. Also notifies the guild, that
    /// the player is logged in. Should therefore be send regularely
    UpdatePlayer,
    /// Queries 51 Hall of Fame entries starting from the top. Starts at 0
    ///
    /// **NOTE:** The server might return less then 51, if there is a "broken"
    /// player encountered. This is NOT a library bug, this is a S&F bug and
    /// will glitch out the UI, when trying to view the page in a browser.
    // I assume this is because the player name contains some invalid
    // character, because in the raw response string the last thing is a
    // half written username "e(" in this case. I would guess that they
    // were created before stricter input validation and never fixed. Might
    // be insightful in the future to use the sequential id lookup in the
    // playerlookat to see, if they can be viewed from there
    HallOfFamePage {
        page: usize,
    },
    /// Queries 51 Hall of Fame entries for the fortress starting from the top.
    /// Starts at 0
    HallOfFameFortressPage {
        page: usize,
    },
    /// Looks at a specific player. Ident is either their name, or player_id.
    /// The information about the player can then be found by using the
    /// lookup_* methods on Otherplayers
    ViewPlayer {
        ident: String,
    },
    /// Buys a beer in the tavern
    BuyBeer,
    /// Starts one of the 3 tavern quests. **0,1,2**
    StartQuest {
        quest_pos: usize,
        overwrite_inv: bool,
    },
    /// Cancels the currently running quest
    CancelQuest,
    /// Finishes the current quest, which starts the battle. This can be used
    /// with a QuestSkip to skip the remaining time
    FinishQuest {
        skip: Option<QuestSkip>,
    },
    /// Goes working for the specified amount of hours (1-10)
    WorkStart {
        hours: u8,
    },
    /// Cancels the current guard job
    WorkCancel,
    /// Collects the pay from the guard job
    WorkFinish,
    /// Checks if the given name is still available to register
    CheckNameAvailable {
        name: String,
    },
    /// Buys a mount, if the player has enough silver/mushrooms
    BuyMount {
        mount: Mount,
    },
    /// Increases the given attribute to the requested number. Should be
    /// current + 1
    IncreaseAttribute {
        attribute: AttributeType,
        increase_to: u32,
    },
    /// Removes the currently active potion 0,1,2
    RemovePotion {
        pos: usize,
    },
    /// Queries the currently available enemies in the arena
    CheckArena,
    /// Fights the selected enemy. This should be used for both arena fights
    /// and normal fights. Not that this actually needs the name, not just the
    /// id
    Fight {
        name: String,
        use_mushroom: bool,
    },
    /// Collects the current reward from the calendar
    CollectCalendar,
    /// Queries information about another guild
    ViewGuild {
        guild_ident: String,
    },
    /// Founds a new guild
    GuildFound {
        name: String,
    },
    /// Invites a player with the given name into the players guild
    GuildInvitePlayer {
        name: String,
    },
    /// Kicks a player with the given name from the players guild
    GuildKickPlayer {
        name: String,
    },
    /// Promote a player from the guild into the leader role
    GuildSetLeader {
        name: String,
    },
    /// Toggles a member between officer and normal member
    GuildToggleOfficer {
        name: String,
    },
    /// Loads a mushroom into the catapult
    GuildLoadMushrooms,
    /// Increases one of the guild skills by 1. Needs to know the current, not
    /// the new  value for some reason
    GuildIncreaseSkill {
        skill: GuildSkill,
        current: u16,
    },
    /// Joins the current ongoing attack
    GuildJoinAttack,
    /// Joins the defense of the guild
    GuildJoinDefense,
    /// Starts an attack in another guild
    GuildAttack {
        guild: String,
    },
    /// Starts the next possible raid
    GuildRaid,
    /// Battles the enemy in the guildportal
    GuildPortalBattle,
    /// Flushes the toilet
    ToiletFlush,
    /// Opens the toilet door for the first time.
    ToiletOpen,
    /// Drops an item from one of the inventories into the toilet
    ToiletDrop {
        inventory: InventoryType,
        pos: usize,
    },
    PlayerPortalBattle,
    /// Buys an item from the shop and puts it in the inventoy slot specified
    BuyShop {
        shop_type: ShopType,
        shop_pos: usize,
        inventory: InventoryType,
        inventory_pos: usize,
    },
    /// Buys an item from the shop and puts it in the inventoy slot specified
    SellShop {
        inventory: InventoryType,
        inventory_pos: usize,
        shop_type: ShopType,
        shop_pos: usize,
    },
    /// Moves an item from one inventory position to another
    InventoryMove {
        inventory_from: InventoryType,
        inventory_from_pos: usize,
        inventory_to: InventoryType,
        inventory_to_pos: usize,
    },
    /// Allows moving items from any position to any other position items can
    /// be at. You should make sure, that the move makes sense (do not move
    /// items from shop to shop)
    ItemMove {
        from: ItemPosition,
        from_pos: usize,
        to: ItemPosition,
        to_pos: usize,
    },
    /// Opens the message at the specified index [0-100]
    MessageOpen {
        index: i32,
    },
    /// Deletes a single message, if you provide the index. -1 = all
    MessageDelete {
        index: i32,
    },
    /// Pulls up your scrapbook to reveal more info, than normal
    ViewScrapbook,
    /// Views a specific pet. This fetches its stats
    ViewPet {
        pet_index: u16,
    },
    /// Unlocks a feature
    UnlockFeature {
        unlockable: Unlockable,
    },
    /// Enters a specific dungeon
    FightLightDungeon {
        name: LightDungeon,
        use_mushroom: bool,
    },
    /// Enters a specific shadow dungeon
    FightShadowDungeon {
        name: ShadowDungeons,
        use_mushroom: bool,
    },
    /// Attacks the requested level of the tower
    FightTower {
        current_level: u8,
        use_mush: bool,
    },
    /// Sets the guild info. Note the info about length limit from
    /// SetDescription
    GuildSetInfo {
        description: String,
        emblem_code: String,
    },
    /// Gambles the desired amount of silver. Picking the right thing is not
    /// actually required. That just masks the determined result
    GambleSilver {
        amount: u64,
    },
    /// Gambles the desired amount of mushrooms. Picking the right thing is not
    /// actually required. That just masks the determined result
    GambleMushrooms {
        amount: u64,
    },
    /// Sends a message to another player
    SendMessage {
        to: String,
        msg: String,
    },
    SetDescription {
        /// The description may only be 240 chars long, when it reaches the
        /// server. The problem is, that special chars like '/' have to get
        /// escaped into two chars "$s" before getting send to the server.
        /// That means this string can be 120-240 chars long depending on the
        /// amount of escaped chars. We 'could' trunctate the response, but
        /// that could get weird with character boundries in UTF8 and split the
        /// escapes themself, so just make sure you provide a valid value here
        /// to begin with and be prepared for a server error
        description: String,
    },
    WitchDropCauldron {
        inventory_t: InventoryType,
        position: usize,
    },
    Blacksmith {
        inventory_t: InventoryType,
        position: u8,
        action: BlacksmithAction,
    },
    GuildSendChat {
        message: String,
    },
    /// Enchants an item, if you have the scroll unlocked. Note that providing
    /// shield here is undefined
    WitchEnchant {
        position: EquipmentSlot,
    },
    SpinWheelOfFortune {
        fortune_payment: FortunePayment,
    },
    /// Collects the reward for collecting points. One of [0,1,2]
    CollectEventTaskReward {
        pos: usize,
    },
    /// Collects the reward for collecting points. One of [0,1,2]
    CollectDailyQuestReward {
        pos: usize,
    },
    EquipCompanion {
        inventory: InventoryType,
        position: u8,
        equipment_slot: EquipmentSlot,
    },
    FortressGather {
        resource: FortressResourceType,
    },
    FortressBuildStart {
        f_type: FortressBuildingType,
    },
    FortressBuildCancel {
        f_type: FortressBuildingType,
    },
    FortressBuildFinish {
        f_type: FortressBuildingType,
        mushrooms: u32,
    },
    /// Builds new units of the selected type
    FortressBuildUnitStart {
        unit: FortressUnitType,
        count: u32,
    },
    /// Starts the search for gems
    FortressGemStoneStart,
    /// Cancles the search for gems
    FortressGemStoneCancel,
    /// Finishes the gem stone search using the appropriate amount of
    /// mushrooms. The price is one mushroom per 600 sec / 10 minutes of time
    /// remaining
    FortressGemStoneFinish {
        mushrooms: u32,
    },
    /// Attacks the current fortress attack target with the provided amount of
    /// soldiers
    FortressAttack {
        soldiers: u32,
    },
    /// Rerolls the enemy in the fortress
    FortressNewEnemy {
        use_mushroom: bool,
    },
    /// Sets the fortress enemy to the counterattack target of the message
    FortressSetCAEnemy {
        msg_id: u32,
    },
    /// Sends a wihsper message to another player
    Whisper {
        player_name: String,
        message: String,
    },
    /// Collects the ressources of the selected type in the underworld
    UnderworldCollect {
        resource: UnderWorldResourceType,
    },
    /// Upgrades the selected underworld unit by one level
    UnderworldUnitUpgrade {
        unit: UnderworldUnitType,
    },
    /// Starts the upgrade of a building in the underworld
    UnderworldUpgradeStart {
        building: UnderworldBuildingType,
        mushrooms: u32,
    },
    /// Cancels the upgrade of a building in the underworld
    UnderworldUpgradeCancel {
        building: UnderworldUnitType,
    },
    /// Finishes an upgrade after the time has run out (or before using
    /// mushrooms)
    UnderworldUpgradeComplete {
        building: UnderworldBuildingType,
        mushrooms: u32,
    },
    /// Lures a player into the underworld
    UnderworldAttack {
        player_id: PlayerId,
    },
    /// Rolls the dice. The first round should be all rerolls, after that,
    /// either reroll again, or take some of the dice on the table
    RollDice {
        payment: RollDiceType,
        dices: [DiceType; 5],
    },
    /// Feeds one of your pets
    PetFeed {
        pet_id: u32,
        fruit_idx: u32,
    },
    /// Fights with the guild pet against the hydra
    GuildPetBattle {
        use_mushroom: bool,
    },
    /// Upgrades an idle building by the requested amount
    IdleUpgrade {
        typ: IdleBuildingType,
        amount: u64,
    },
    /// Sacrifice all the money in the idle game for runes
    IdleSacrifice,
    /// Upgrades a skill to the requested atribute. Should probably be just
    /// current + 1 to mimic a user clicking
    UpgradeSkill {
        attribute: AttributeType,
        next_attribute: u32,
    },
    /// Spend 1 mushroom to update the inventory of a shop
    RefreshShop {
        shop: ShopType,
    },
    /// Fetches the HoF page for guilds
    HallOfFameGroupPage {
        page: u32,
    },
    /// Crawls the HoF page for the underworld
    HallOfFameUnderworldPage {
        page: u32,
    },
    HallOfFamePetsPage {
        page: u32,
    },
    /// Switch equipment with the manequin, if it is unlocked
    SwapManequin,
    /// Updates your flag in the HoF
    UpdateFlag {
        flag: Option<Flag>,
    },
    /// Changes if you can receive invites or not
    BlockGuildInvites {
        block_invites: bool,
    },
    /// Changes if you want to gets tips in the gui. Does nothing for the API
    ShowTips {
        show_tips: bool,
    },
    /// Change your password. Note that I have not tested this and this might
    /// invalidate your session
    ChangePassword {
        old: String,
        new: String,
    },
    /// Changes your mail to another address
    ChangeMailAddress {
        old_mail: String,
        new_mail: String,
        password: String,
        username: String,
    },
    /// Sets the language of the character. This should be basically
    /// irrelevant, but is still included for completeness sake. Expects a
    /// valid county code. I have not tested all, but it should be one of:
    /// `ru,fi,ar,tr,nl,ja,it,sk,fr,ko,pl,cs,el,da,en,hr,de,zh,sv,hu,pt,es,
    /// pt-br, ro`
    SetLanguage {
        language: String,
    },
    /// Sets the relation to another player
    SetPlayerRelation {
        player_id: PlayerId,
        relation: Relationship,
    },
    /// I have no character with anything but the default (0) to test this
    /// with. If I had to guess, they continue sequentially
    SetPortraitFrame {
        portrait_id: i64,
    },
    /// Swaps the runes of two items
    SwapRunes {
        from: ItemPosition,
        from_pos: usize,
        to: ItemPosition,
        to_pos: usize,
    },
    /// Changes the look of the item to the selected raw_model_id for 10
    /// mushrooms. Note that this is NOT the normal model id. it is the
    /// model_id  + (class as usize) * 1000 if I remember correctly. Pretty
    /// sure nobody  will ever uses this though, as it is only for looks.
    ChangeItemLook {
        inv: ItemPosition,
        pos: usize,
        raw_model_id: u16,
    },
    /// Continues the expedition on one of the three streets, [0,1,2]
    ExpeditionChooseStreet {
        pos: usize,
    },
    /// Continues the expedition, if you are currently in a situation, where
    /// there is only one option. This can be starting a fighting, or starting
    /// the wait after a fight (collecting the non item reward)
    ExpeditionContinue,
    /// If there are multiple items to choose from after fighting a boss, you
    /// can choose which one to take here. [0,1,2]
    ExpeditionPickItem {
        pos: usize,
    },
    /// Starts one of the two expeditions [0,1]
    ExpeditionStart {
        pos: usize,
    },
}
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlacksmithAction {
    Dismantle = 201,
    SocketUpgrade = 202,
    SocketUpgradeWithMushrooms = 212,
    GemExtract = 203,
    GemExtractWithMushrooms = 213,
    Upgrade = 204,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortunePayment {
    LuckyCoins = 0,
    Mushrooms,
    FreeTurn,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RollDiceType {
    Free = 0,
    Mushrooms,
    Hourglass,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiceType {
    ReRoll,
    Silver,
    Stone,
    Wood,
    Souls,
    Arcane,
    Hourglass,
}
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiceReward {
    pub win_typ: DiceType,
    pub amount: u32,
}

#[derive(
    Debug, Copy, Clone, strum::EnumCount, PartialEq, Eq, Enum, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AttributeType {
    Strength = 1,
    Dexterity = 2,
    Intelligence = 3,
    Constitution = 4,
    Luck = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShopType {
    Weapon = 3,
    Magic = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QuestSkip {
    Mushroom = 1,
    Glass = 2,
}

impl Command {
    /// Returns the unencrypted string, that has to be send to the server to to
    /// perform the request
    #[allow(deprecated, clippy::useless_format)]
    pub(crate) fn request_string(&self) -> String {
        const APP_VERSION: &str = "1800000000000";
        use Command::*;
        match self {
            Custom { cmd_name, values } => {
                format!("{cmd_name}:{}", values.join("/"))
            }
            Login {
                username,
                pw_hash,
                login_count,
            } => {
                let full_hash = sha1_hash(&format!("{pw_hash}{login_count}"));
                format!(
                    "AccountLogin:{username}/{full_hash}/{login_count}/\
                     unity3d_webglplayer//{APP_VERSION}///0/"
                )
            }
            #[cfg(feature = "sso")]
            SSOLogin {
                uuid, character_id, ..
            } => format!(
                "SFAccountCharLogin:{uuid}/{character_id}/unity3d_webglplayer/\
                 /{APP_VERSION}"
            ),
            Register {
                username,
                password,
                gender,
                race,
                class,
            } => {
                // TODO: Custom portrait
                format!(
                    "AccountCreate:{username}/{password}/{username}@playa.sso/\
                     {}/{}/{}/8,203,201,6,199,3,1,2,1/0//en",
                    *gender as usize + 1,
                    *race as usize,
                    *class as usize + 1
                )
            }
            UpdatePlayer => "Poll:".to_string(),
            HallOfFamePage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("PlayerGetHallOfFame:{pos}//25/25")
            }
            HallOfFameFortressPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("FortressGetHallOfFame:{pos}//25/25")
            }
            HallOfFameGroupPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("GroupGetHallOfFame:{pos}//25/25")
            }
            HallOfFameUnderworldPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("UnderworldGetHallOfFame:{pos}//25/25")
            }
            HallOfFamePetsPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("PetsGetHallOfFame:{pos}//25/25")
            }
            ViewPlayer { ident } => format!("PlayerLookAt:{ident}"),
            BuyBeer => format!("PlayerBeerBuy:"),
            StartQuest {
                quest_pos,
                overwrite_inv,
            } => {
                format!(
                    "PlayerAdventureStart:{}/{}",
                    quest_pos + 1,
                    *overwrite_inv as u8
                )
            }
            CancelQuest => format!("PlayerAdventureStop:"),
            FinishQuest { skip } => {
                format!(
                    "PlayerAdventureFinished:{}",
                    skip.map(|a| a as u8).unwrap_or(0)
                )
            }
            WorkStart { hours } => format!("PlayerWorkStart:{hours}"),
            WorkCancel => format!("PlayerWorkStop:"),
            WorkFinish => format!("PlayerWorkFinished:"),
            CheckNameAvailable { name } => format!("AccountCheck:{name}"),
            BuyMount { mount } => format!("PlayerMountBuy:{}", *mount as usize),
            IncreaseAttribute {
                attribute,
                increase_to,
            } => format!(
                "PlayerAttributIncrease:{}/{increase_to}",
                *attribute as u8
            ),
            RemovePotion { pos } => format!("PlayerPotionKill:{}", pos + 1),
            CheckArena => format!("PlayerArenaEnemy:"),
            Fight { name, use_mushroom } => {
                format!("PlayerArenaFight:{name}/{}", *use_mushroom as u8)
            }
            CollectCalendar => format!("PlayerOpenCalender:"),
            UpgradeSkill {
                attribute,
                next_attribute,
            } => format!(
                "PlayerAttributIncrease:{}/{next_attribute}",
                *attribute as i64
            ),
            RefreshShop { shop } => {
                format!("PlayerNewWares:{}", *shop as usize - 2)
            }
            ViewGuild { guild_ident } => {
                format!("GroupLookAt:{guild_ident}")
            }
            GuildFound { name } => format!("GroupFound:{name}"),
            GuildInvitePlayer { name } => format!("GroupInviteMember:{name}"),
            GuildKickPlayer { name } => format!("GroupRemoveMember:{name}"),
            GuildSetLeader { name } => format!("GroupSetLeader:{name}"),
            GuildToggleOfficer { name } => format!("GroupSetOfficer:{name}"),
            GuildLoadMushrooms => {
                format!("GroupIncreaseBuilding:0")
            }
            GuildIncreaseSkill { skill, current } => {
                format!("GroupSkillIncrease:{}/{current}", *skill as usize)
            }
            GuildJoinAttack => format!("GroupReadyAttack:"),
            GuildJoinDefense => format!("GroupReadyDefense:"),
            GuildAttack { guild } => format!("GroupAttackDeclare:{guild}"),
            GuildRaid => format!("GroupRaidDeclare:"),
            ToiletFlush => format!("PlayerToilettFlush:"),
            ToiletOpen => format!("PlayerToilettOpenWithKey:"),
            FightTower {
                current_level: progress,
                use_mush,
            } => {
                format!("PlayerTowerBattle:{progress}/{}", *use_mush as u8)
            }
            ToiletDrop { inventory, pos } => {
                format!("PlayerToilettLoad:{}/{}", *inventory as usize, pos + 1)
            }
            GuildPortalBattle => format!("GroupPortalBattle:"),
            PlayerPortalBattle => format!("PlayerPortalBattle:"),
            MessageOpen { index } => {
                format!("PlayerMessageView:{}", *index + 1)
            }
            MessageDelete { index } => format!(
                "PlayerMessageDelete:{}",
                match index {
                    -1 => -1,
                    x => *x + 1,
                }
            ),
            ViewScrapbook => format!("PlayerPollScrapbook:"),
            ViewPet { pet_index } => format!("PetsGetStats:{pet_index}"),
            BuyShop {
                shop_type,
                shop_pos,
                inventory,
                inventory_pos,
            } => format!(
                "PlayerItemMove:{}/{}/{}/{}",
                *shop_type as usize,
                *shop_pos + 1,
                *inventory as usize,
                *inventory_pos + 1
            ),
            SellShop {
                inventory,
                inventory_pos,
                shop_type,
                shop_pos,
            } => format!(
                "PlayerItemMove:{}/{}/{}/{}",
                *inventory as usize,
                *inventory_pos + 1,
                *shop_type as usize,
                *shop_pos + 1,
            ),
            InventoryMove {
                inventory_from,
                inventory_from_pos,
                inventory_to,
                inventory_to_pos,
            } => format!(
                "PlayerItemMove:{}/{}/{}/{}",
                *inventory_from as usize,
                *inventory_from_pos + 1,
                *inventory_to as usize,
                *inventory_to_pos + 1
            ),
            ItemMove {
                from,
                from_pos,
                to,
                to_pos,
            } => format!(
                "PlayerItemMove:{}/{}/{}/{}",
                *from as usize,
                *from_pos + 1,
                *to as usize,
                *to_pos + 1
            ),
            UnlockFeature { unlockable } => format!(
                "UnlockFeature:{}/{}",
                unlockable.main_ident, unlockable.sub_ident
            ),
            FightLightDungeon { name, use_mushroom } => format!(
                "PlayerDungeonBattle:{}/{}",
                *name as usize + 1,
                if *use_mushroom { 1 } else { 2 }
            ),
            GuildSetInfo {
                description,
                emblem_code,
            } => format!(
                "GroupSetDescription:{emblem_code}§{}",
                to_sf_string(description)
            ),
            SetDescription { description } => {
                format!("PlayerSetDescription:{}", &to_sf_string(description))
            }
            GuildSendChat { message } => {
                format!("GroupChat:{}", &to_sf_string(message))
            }
            GambleSilver { amount } => format!("PlayerGambleGold:{amount}"),
            GambleMushrooms { amount } => format!("PlayerGambleCoins:{amount}"),
            SendMessage { to, msg } => {
                format!("PlayerMessageSend:{to}/{}", to_sf_string(msg))
            }
            WitchDropCauldron {
                inventory_t,
                position,
            } => format!(
                "PlayerWitchSpendItem:{}/{}",
                *inventory_t as usize,
                position + 1
            ),
            Blacksmith {
                inventory_t,
                position,
                action,
            } => format!(
                "PlayerItemMove:{}/{}/{}/-1",
                *inventory_t as usize,
                position + 1,
                *action as usize
            ),
            WitchEnchant { position } => {
                format!("PlayerWitchEnchantItem:{}/1", position.witch_id())
            }
            SpinWheelOfFortune { fortune_payment } => {
                format!("WheelOfFortune:{}", *fortune_payment as usize)
            }
            FortressGather { resource } => {
                format!("FortressGather:{}", *resource as usize + 1)
            }
            EquipCompanion {
                inventory,
                position,
                equipment_slot,
            } => format!(
                "PlayerItemMove:{}/{}/1/{}",
                *inventory as usize,
                position + 1,
                *equipment_slot as usize
            ),
            FortressBuildStart { f_type } => {
                format!("FortressBuildStart:{}/0", *f_type as usize + 1)
            }
            FortressBuildCancel { f_type } => {
                format!("FortressBuildStop:{}", *f_type as usize + 1)
            }
            FortressBuildFinish { f_type, mushrooms } => format!(
                "FortressBuildFinish:{}/{mushrooms}",
                *f_type as usize + 1
            ),
            FortressBuildUnitStart { unit, count } => {
                format!("FortressBuildUnitStart:{}/{count}", *unit as usize + 1)
            }
            FortressGemStoneStart => format!("FortressGemstoneStart:",),
            FortressGemStoneCancel => format!("FortressGemStoneStop:0"),
            FortressGemStoneFinish { mushrooms } => {
                format!("FortressGemstoneFinished:{mushrooms}",)
            }
            FortressAttack { soldiers } => format!("FortressAttack:{soldiers}"),
            FortressNewEnemy { use_mushroom: pay } => {
                format!("FortressEnemy:{}", *pay as usize)
            }
            FortressSetCAEnemy { msg_id } => {
                format!("FortressEnemy:0/{}", *msg_id)
            }
            Whisper {
                player_name: player,
                message,
            } => format!(
                "PlayerMessageWhisper:{}/{}",
                player,
                to_sf_string(message)
            ),
            UnderworldCollect {
                resource: resource_t,
            } => {
                format!("UnderworldGather:{}", *resource_t as usize + 1)
            }
            UnderworldUnitUpgrade { unit: unit_t } => {
                format!("UnderworldUpgradeUnit:{}", *unit_t as usize + 1)
            }
            UnderworldUpgradeStart {
                building,
                mushrooms,
            } => format!(
                "UnderworldBuildStart:{}/{mushrooms}",
                *building as usize + 1
            ),
            UnderworldUpgradeCancel { building } => {
                format!("UnderworldBuildStop:{}", *building as usize + 1)
            }
            UnderworldUpgradeComplete {
                building,
                mushrooms,
            } => format!(
                "UnderworldBuildFinished:{}/{mushrooms}",
                *building as usize + 1
            ),
            UnderworldAttack { player_id } => {
                format!("UnderworldAttack:{player_id}")
            }
            RollDice { payment, dices } => {
                let mut dices =
                    dices.iter().fold("".to_string(), |mut a, b| {
                        if !a.is_empty() {
                            a.push('/')
                        }
                        a.push((*b as u8 + b'0') as char);
                        a
                    });

                if dices.is_empty() {
                    dices = "0/0/0/0/0".to_string()
                }
                format!("RollDice:{}/{}", *payment as usize, dices)
            }
            PetFeed { pet_id, fruit_idx } => {
                format!("PlayerPetFeed:{pet_id}/{fruit_idx}")
            }
            GuildPetBattle { use_mushroom } => {
                format!("GroupPetBattle:{}", *use_mushroom as usize)
            }
            IdleUpgrade { typ: kind, amount } => {
                format!("IdleIncrease:{}/{}", *kind as usize, amount)
            }
            IdleSacrifice => format!("IdlePrestige:0"),
            SwapManequin => format!("PlayerDummySwap:301/1"),
            UpdateFlag { flag } => format!(
                "PlayerSetFlag:{}",
                flag.map(|a| a.code()).unwrap_or_default()
            ),
            BlockGuildInvites { block_invites } => {
                format!("PlayerSetNoGroupInvite:{}", *block_invites as u8)
            }
            ShowTips { show_tips } => format!(
                "PlayerTutorialStatus:{}",
                if *show_tips { 0 } else { 268435455 }
            ),
            ChangePassword { old, new } => {
                let old = sha1_hash(&format!("{}{}", old, HASH_CONST));
                let new = sha1_hash(&format!("{}{}", new, HASH_CONST));
                format!("AccountPasswordChange:Lexi Belle/{old}/106/{new}/")
            }
            ChangeMailAddress {
                old_mail,
                new_mail,
                password,
                username,
            } => {
                let pass = sha1_hash(&format!("{}{}", password, HASH_CONST));
                format!(
                    "AccountMailChange:{old_mail}/{new_mail}/{username}/\
                     {pass}/106"
                )
            }
            SetLanguage { language } => {
                format!("AccountSetLanguage:{language}")
            }
            SetPlayerRelation {
                player_id,
                relation,
            } => format!("PlayerFriendSet:{player_id}/{}", *relation as i32),
            SetPortraitFrame { portrait_id } => {
                format!("PlayerSetActiveFrame:{portrait_id}")
            }
            CollectDailyQuestReward { pos } => {
                format!("DailyTaskClaim:1/{}", pos + 1)
            }
            CollectEventTaskReward { pos } => {
                format!("DailyTaskClaim:2/{}", pos + 1)
            }
            SwapRunes {
                from,
                from_pos,
                to,
                to_pos,
            } => {
                format!(
                    "PlayerSmithSwapRunes:{}/{}/{}/{}",
                    *from as usize,
                    *from_pos + 1,
                    *to as usize,
                    *to_pos + 1
                )
            }
            ChangeItemLook {
                inv,
                pos,
                raw_model_id: model_id,
            } => {
                format!(
                    "ItemChangePicture:{}/{}/{}",
                    *inv as usize,
                    pos + 1,
                    model_id
                )
            }
            ExpeditionChooseStreet { pos } => {
                format!("ExpeditionProceed:{}", pos + 1)
            }
            ExpeditionContinue => format!("ExpeditionProceed:1"),
            ExpeditionPickItem { pos } => {
                format!("ExpeditionProceed:{}", pos + 1)
            }
            ExpeditionStart { pos } => format!("ExpeditionStart:{}", pos + 1),
            FightShadowDungeon { name, use_mushroom } => format!(
                "PlayerShadowBattle:{}/{}",
                *name as u32 + 1,
                *use_mushroom as u8
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Flag {
    Australia,
    Austria,
    Belgium,
    Brazil,
    Bulgaria,
    Canada,
    Chile,
    China,
    Czechia,
    Denmark,
    Finland,
    France,
    Germany,
    GreatBritain,
    Greece,
    Hungary,
    India,
    Italy,
    Japan,
    Lithuania,
    Mexico,
    Netherlands,
    Peru,
    Philippines,
    Poland,
    Portugal,
    Romania,
    Russia,
    SaudiArabia,
    Slovakia,
    SouthKorea,
    Spain,
    Sweden,
    Switzerland,
    Thailand,
    Turkey,
    Ukraine,
    UnitedArabEmirates,
    UnitedStates,
    Vietnam,
}
impl Flag {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Flag::Australia => "au",
            Flag::Austria => "at",
            Flag::Belgium => "be",
            Flag::Brazil => "br",
            Flag::Bulgaria => "bu",
            Flag::Canada => "ca",
            Flag::Chile => "cl",
            Flag::China => "cn",
            Flag::Czechia => "cz",
            Flag::Denmark => "dk",
            Flag::Finland => "fi",
            Flag::France => "fr",
            Flag::Germany => "de",
            Flag::GreatBritain => "gb",
            Flag::Greece => "gr",
            Flag::Hungary => "hu",
            Flag::India => "in",
            Flag::Italy => "it",
            Flag::Japan => "jp",
            Flag::Lithuania => "lt",
            Flag::Mexico => "mx",
            Flag::Netherlands => "nl",
            Flag::Peru => "pe",
            Flag::Philippines => "ph",
            Flag::Poland => "pl",
            Flag::Portugal => "pt",
            Flag::Romania => "ro",
            Flag::Russia => "ru",
            Flag::SaudiArabia => "sa",
            Flag::Slovakia => "sk",
            Flag::SouthKorea => "kr",
            Flag::Spain => "es",
            Flag::Sweden => "se",
            Flag::Switzerland => "ch",
            Flag::Thailand => "th",
            Flag::Turkey => "tr",
            Flag::Ukraine => "ua",
            Flag::UnitedArabEmirates => "ae",
            Flag::UnitedStates => "us",
            Flag::Vietnam => "vn",
        }
    }
}
