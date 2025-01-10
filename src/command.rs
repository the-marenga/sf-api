#![allow(deprecated)]
use enum_map::Enum;
use log::warn;
use num_derive::FromPrimitive;
use strum::EnumIter;

use crate::{
    gamestate::{
        character::*,
        dungeons::{CompanionClass, Dungeon},
        fortress::*,
        guild::{Emblem, GuildSkill},
        idle::IdleBuildingType,
        items::*,
        social::Relationship,
        underworld::*,
        unlockables::{HabitatType, HellevatorTreatType, Unlockable},
    },
    PlayerId,
};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A command, that can be send to the sf server
pub enum Command {
    /// If there is a command you somehow know/reverse engineered, or need to
    /// extend the functionality of one of the existing commands, this is the
    /// command for you
    Custom {
        /// The thing in the command, that comes before the ':'
        cmd_name: String,
        /// The values this command gets as arguments. These will be joines
        /// with '/'
        arguments: Vec<String>,
    },
    /// Manually sends a login request to the server.
    /// **WARN:** The behaviour for a credentials mismatch, with the
    /// credentials in the user is undefined. Use the login method instead
    /// for a safer abstraction
    #[deprecated = "Use the login method instead"]
    Login {
        /// The username of the player you are trying to login
        username: String,
        /// The sha1 hashed password of the player
        pw_hash: String,
        /// Honestly, I am not 100% sure what this is anymore, but it is
        /// related to the maount of times you have logged in. Might be useful
        /// for logging in again after error
        login_count: u32,
    },
    #[cfg(feature = "sso")]
    /// Manually sends a login request to the server.
    /// **WARN:** The behaviour for a credentials mismatch, with the
    /// credentials in the user is undefined. Use the login method instead for
    /// a safer abstraction
    #[deprecated = "Use a login method instead"]
    SSOLogin {
        /// The Identifies the S&F account, that has this character
        uuid: String,
        /// Identifies the specific character an account has
        character_id: String,
        /// The thing to authenticate with
        bearer_token: String,
    },
    /// Registers a new normal character in the server. I am not sure about the
    /// portrait, so currently this sets the same default portrait for every
    /// char
    #[deprecated = "Use the register method instead"]
    Register {
        /// The username of the new account
        username: String,
        /// The password of the new account
        password: String,
        /// The gender of the new character
        gender: Gender,
        /// The race of the new character
        race: Race,
        /// The class of the new character
        class: Class,
    },
    /// Updates the current state of the entire gamestate. Also notifies the
    /// guild, that the player is logged in. Should therefore be send
    /// regularely
    Update,
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
        /// The page of the Hall of Fame you want to query.
        ///
        /// 0 => rank(0..=50), 1 => rank(51..=101), ...
        page: usize,
    },
    /// Queries 51 Hall of Fame entries for the fortress starting from the top.
    /// Starts at 0
    HallOfFameFortressPage {
        /// The page of the Hall of Fame you want to query.
        ///
        /// 0 => rank(0..=50), 1 => rank(51..=101), ...
        page: usize,
    },
    /// Looks at a specific player. Ident is either their name, or `player_id`.
    /// The information about the player can then be found by using the
    /// lookup_* methods on `HallOfFames`
    ViewPlayer {
        /// Either the name, or the `playerid.to_string()`
        ident: String,
    },
    /// Buys a beer in the tavern
    BuyBeer,
    /// Starts one of the 3 tavern quests. **0,1,2**
    StartQuest {
        /// The position of the quest in the quest array
        quest_pos: usize,
        /// Has the player acknowledged, that their inventory is full and this
        /// may lead to the loss of an item?
        overwrite_inv: bool,
    },
    /// Cancels the currently running quest
    CancelQuest,
    /// Finishes the current quest, which starts the battle. This can be used
    /// with a `QuestSkip` to skip the remaining time
    FinishQuest {
        /// If this is `Some()`, it will use the selected skip to skip the
        /// remaining quest wait
        skip: Option<TimeSkip>,
    },
    /// Goes working for the specified amount of hours (1-10)
    StartWork {
        /// The amount of hours you want to work
        hours: u8,
    },
    /// Cancels the current guard job
    CancelWork,
    /// Collects the pay from the guard job
    FinishWork,
    /// Checks if the given name is still available to register
    CheckNameAvailable {
        /// The name to check
        name: String,
    },
    /// Buys a mount, if the player has enough silver/mushrooms
    BuyMount {
        /// The mount you want to buy
        mount: Mount,
    },
    /// Increases the given base attribute to the requested number. Should be
    /// `current + 1`
    IncreaseAttribute {
        /// The attribute you want to increase
        attribute: AttributeType,
        /// The value you increase it to. This should be `current + 1`
        increase_to: u32,
    },
    /// Removes the currently active potion 0,1,2
    RemovePotion {
        /// The position of the posion you want to remove
        pos: usize,
    },
    /// Queries the currently available enemies in the arena
    CheckArena,
    /// Fights the selected enemy. This should be used for both arena fights
    /// and normal fights. Note that this actually needs the name, not just the
    /// id
    Fight {
        /// The name of the player you want to fight
        name: String,
        /// If the arena timer has not elapsed yet, this will spend a mushroom
        /// and fight regardless. Currently the server ignores this and fights
        /// always, but the client sends the correctly set command, so you
        /// should too
        use_mushroom: bool,
    },
    /// Collects the current reward from the calendar
    CollectCalendar,
    /// Queries information about another guild. The information can bet found
    /// in `hall_of_fames.other_guilds`
    ViewGuild {
        /// Either the id, or name of the guild you want to look at
        guild_ident: String,
    },
    /// Founds a new guild
    GuildFound {
        /// The name of the new guild you want to found
        name: String,
    },
    /// Invites a player with the given name into the players guild
    GuildInvitePlayer {
        /// The name of the player you want to invite
        name: String,
    },
    /// Kicks a player with the given name from the players guild
    GuildKickPlayer {
        /// The name of the guild member you want to kick
        name: String,
    },
    /// Promote a player from the guild into the leader role
    GuildSetLeader {
        /// The name of the guild member you want to set as the guild leader
        name: String,
    },
    /// Toggles a member between officer and normal member
    GuildToggleOfficer {
        /// The name of the player you want to toggle the officer status for
        name: String,
    },
    /// Loads a mushroom into the catapult
    GuildLoadMushrooms,
    /// Increases one of the guild skills by 1. Needs to know the current, not
    /// the new value for some reason
    GuildIncreaseSkill {
        /// The skill you want to increase
        skill: GuildSkill,
        /// The current value of the guild skill
        current: u16,
    },
    /// Joins the current ongoing attack
    GuildJoinAttack,
    /// Joins the defense of the guild
    GuildJoinDefense,
    /// Starts an attack in another guild
    GuildAttack {
        /// The name of the guild you want to attack
        guild: String,
    },
    /// Starts the next possible raid
    GuildRaid,
    /// Battles the enemy in the guildportal
    GuildPortalBattle,
    /// Fetch the fightable guilds
    GuildGetFightableTargets,
    /// Flushes the toilet
    ToiletFlush,
    /// Opens the toilet door for the first time.
    ToiletOpen,
    /// Drops an item from one of the inventories into the toilet
    ToiletDrop {
        /// The inventory you want to take the item from
        inventory: PlayerItemPlace,
        /// The position of the item in the inventory. Starts at 0
        pos: usize,
    },
    /// Buys an item from the shop and puts it in the inventoy slot specified
    BuyShop {
        /// The shop you want to buy from
        shop_type: ShopType,
        /// the position of the item you want to buy from the shop
        shop_pos: usize,
        /// The inventory you want to put the new item into
        inventory: PlayerItemPlace,
        /// The position in the chosen inventory you
        inventory_pos: usize,
    },
    /// Sells an item from the players inventory. To make this more convenient,
    /// this picks a shop&item position to sell to for you
    SellShop {
        /// The inventory you want to sell an item from
        inventory: PlayerItemPlace,
        /// The position of the item you want to sell
        inventory_pos: usize,
    },
    /// Moves an item from one inventory position to another
    InventoryMove {
        /// The inventory you move the item from
        inventory_from: PlayerItemPlace,
        /// The position of the item you want to move
        inventory_from_pos: usize,
        /// The inventory you move the item to
        inventory_to: PlayerItemPlace,
        /// The inventory you move the item from
        inventory_to_pos: usize,
    },
    /// Allows moving items from any position to any other position items can
    /// be at. You should make sure, that the move makes sense (do not move
    /// items from shop to shop)
    ItemMove {
        /// The place of thing you move the item from
        from: ItemPlace,
        /// The position of the item you want to move
        from_pos: usize,
        /// The place of thing you move the item to
        to: ItemPlace,
        /// The position of the item you want to move
        to_pos: usize,
    },
    /// Allows using an potion from any position
    UsePotion {
        /// The place of the potion you use from
        from: ItemPlace,
        /// The position of the potion you want to use
        from_pos: usize,
    },
    /// Opens the message at the specified index [0-100]
    MessageOpen {
        /// The index of the message in the inbox vec
        pos: i32,
    },
    /// Deletes a single message, if you provide the index. -1 = all
    MessageDelete {
        /// The position of the message to delete in the inbox vec. If this is
        /// -1, it deletes all
        pos: i32,
    },
    /// Pulls up your scrapbook to reveal more info, than normal
    ViewScrapbook,
    /// Views a specific pet. This fetches its stats and places it into the
    /// specified pet in the habitat
    ViewPet {
        /// The id of the pet, that you want to view
        pet_id: u16,
    },
    /// Unlocks a feature. The these unlockables can be found in
    /// `pending_unlocks` on `GameState`
    UnlockFeature {
        /// The thing to unlock
        unlockable: Unlockable,
    },
    /// Starts a fight against the enemy in the players portal
    FightPortal,
    /// Enters a specific dungeon. This works for all dungeons, except the
    /// Tower, which you must enter via the `FightTower` command
    FightDungeon {
        /// The dungeon you want to fight in (except the tower). If you only
        /// have a `LightDungeon`, or `ShadowDungeon`, you need to call
        /// `into()` to turn them into a generic dungeon
        dungeon: Dungeon,
        /// If this is true, you will spend a mushroom, if the timer has not
        /// run out. Note, that this is currently ignored by the server for
        /// some reason
        use_mushroom: bool,
    },
    /// Attacks the requested level of the tower
    FightTower {
        /// The current level you are on the tower
        current_level: u8,
        /// If this is true, you will spend a mushroom, if the timer has not
        /// run out. Note, that this is currently ignored by the server for
        /// some reason
        use_mush: bool,
    },
    /// Fights the player opponent with your pet
    FightPetOpponent {
        /// The habitat opponent you want to attack the opponent in
        habitat: HabitatType,
        /// The id of the player you want to fight
        opponent_id: PlayerId,
    },
    /// Fights the pet in the specified habitat dungeon
    FightPetDungeon {
        /// If this is true, you will spend a mushroom, if the timer has not
        /// run out. Note, that this is currently ignored by the server for
        /// some reason
        use_mush: bool,
        /// The habitat, that you want to fight in
        habitat: HabitatType,
        /// This is `explored + 1` of the given habitat. Note that 20 explored
        /// is the max, so providing 21 here will return an err
        enemy_pos: u32,
        /// This `pet_id` is the id of the pet you want to send into battle.
        /// The pet has to be from the same habitat, as the dungeon you are
        /// trying
        player_pet_id: u32,
    },
    /// Sets the guild info. Note the info about length limit from
    /// `SetDescription` for the description
    GuildSetInfo {
        /// The description you want to set
        description: String,
        /// The emblem you want to set
        emblem: Emblem,
    },
    /// Gambles the desired amount of silver. Picking the right thing is not
    /// actually required. That just masks the determined result. The result
    /// will be in `gamble_result` on `Tavern`
    GambleSilver {
        /// The amount of silver to gamble
        amount: u64,
    },
    /// Gambles the desired amount of mushrooms. Picking the right thing is not
    /// actually required. That just masks the determined result. The result
    /// will be in `gamble_result` on `Tavern`
    GambleMushrooms {
        /// The amount of mushrooms to gamble
        amount: u64,
    },
    /// Sends a message to another player
    SendMessage {
        /// The name of the player to send a message to
        to: String,
        /// The message to send
        msg: String,
    },
    /// The description may only be 240 chars long, when it reaches the
    /// server. The problem is, that special chars like '/' have to get
    /// escaped into two chars "$s" before getting send to the server.
    /// That means this string can be 120-240 chars long depending on the
    /// amount of escaped chars. We 'could' truncate the response, but
    /// that could get weird with character boundaries in UTF8 and split the
    /// escapes themself, so just make sure you provide a valid value here
    /// to begin with and be prepared for a server error
    SetDescription {
        /// The description to set
        description: String,
    },
    /// Drop the item from the specified position into the witches cauldron
    WitchDropCauldron {
        /// The inventory you want to move an item from
        inventory_t: PlayerItemPlace,
        /// The position of the item to move
        position: usize,
    },
    /// Uses the blacksmith with the specified action on the specified item
    Blacksmith {
        /// The inventory the item you want to act upon is in
        inventory_t: PlayerItemPlace,
        /// The position of the item in the inventory
        position: u8,
        /// The action you want to use on the item
        action: BlacksmithAction,
    },
    /// Sends the specified message in the guild chat
    GuildSendChat {
        /// The message to send
        message: String,
    },
    /// Enchants the currently worn item, associated with this enchantment,
    /// with the enchantment
    WitchEnchant {
        /// The enchantment to apply
        enchantment: Enchantment,
    },
    /// Spins the wheel. All information about when you can spin, or what you
    /// won are in `game_state.specials.wheel`
    SpinWheelOfFortune {
        /// The resource you want to spend to spin the wheel
        payment: FortunePayment,
    },
    /// Collects the reward for event points
    CollectEventTaskReward {
        /// One of [0,1,2], depending on which reward has been unlocked
        pos: usize,
    },
    /// Collects the reward for collecting points.
    CollectDailyQuestReward {
        /// One of [0,1,2], depending on which chest you want to collect
        pos: usize,
    },
    /// Moves an item from a normal inventory, onto one of the companions
    EquipCompanion {
        /// The inventory of your character you take the item from
        from_inventory: InventoryType,
        /// The position in the inventory, that you
        from_pos: u8,
        /// The companion you want to equip
        to_companion: CompanionClass,
        /// The slot of the companion you want to equip
        to_slot: EquipmentSlot,
    },
    /// Collects a specific resource from the fortress
    FortressGather {
        /// The type of resource you want to collect
        resource: FortressResourceType,
    },
    /// Builds, or upgrades a building in the fortress
    FortressBuild {
        /// The building you want to upgrade, or build
        f_type: FortressBuildingType,
    },
    /// Cancels the current build/upgrade, of the specified building in the
    /// fortress
    FortressBuildCancel {
        /// The building you want to cancel the upgrade, or build of
        f_type: FortressBuildingType,
    },
    /// Finish building/upgrading a Building
    /// When mushrooms != 0, mushrooms will be used to "skip" the upgrade timer.
    /// However, this command also needs to be sent when not skipping the wait,
    /// with mushrooms = 0, after the build/upgrade timer has finished.
    FortressBuildFinish {
        f_type: FortressBuildingType,
        mushrooms: u32,
    },
    /// Builds new units of the selected type
    FortressBuildUnit {
        unit: FortressUnitType,
        count: u32,
    },
    /// Starts the search for gems
    FortressGemStoneSearch,
    /// Cancels the search for gems
    FortressGemStoneSearchCancel,
    /// Finishes the gem stone search using the appropriate amount of
    /// mushrooms. The price is one mushroom per 600 sec / 10 minutes of time
    /// remaining
    FortressGemStoneSearchFinish {
        mushrooms: u32,
    },
    /// Attacks the current fortress attack target with the provided amount of
    /// soldiers
    FortressAttack {
        soldiers: u32,
    },
    /// Re-rolls the enemy in the fortress
    FortressNewEnemy {
        use_mushroom: bool,
    },
    /// Sets the fortress enemy to the counterattack target of the message
    FortressSetCAEnemy {
        msg_id: u32,
    },
    /// Upgrades the Hall of Knights to the next level
    FortressUpgradeHallOfKnights,
    /// Sends a whisper message to another player
    Whisper {
        player_name: String,
        message: String,
    },
    /// Collects the resources of the selected type in the underworld
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
    UnderworldUpgradeFinish {
        building: UnderworldBuildingType,
        mushrooms: u32,
    },
    /// Lures a player into the underworld
    UnderworldAttack {
        player_id: PlayerId,
    },
    /// Rolls the dice. The first round should be all re-rolls, after that,
    /// either re-roll again, or take some of the dice on the table
    RollDice {
        payment: RollDicePrice,
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
    /// Upgrades a skill to the requested attribute. Should probably be just
    /// current + 1 to mimic a user clicking
    UpgradeSkill {
        attribute: AttributeType,
        next_attribute: u32,
    },
    /// Spend 1 mushroom to update the inventory of a shop
    RefreshShop {
        shop: ShopType,
    },
    /// Fetches the Hall of Fame page for guilds
    HallOfFameGroupPage {
        page: u32,
    },
    /// Crawls the Hall of Fame page for the underworld
    HallOfFameUnderworldPage {
        page: u32,
    },
    HallOfFamePetsPage {
        page: u32,
    },
    /// Switch equipment with the manequin, if it is unlocked
    SwapManequin,
    /// Updates your flag in the Hall of Fame
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
        username: String,
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
        from: ItemPlace,
        from_pos: usize,
        to: ItemPlace,
        to_pos: usize,
    },
    /// Changes the look of the item to the selected `raw_model_id` for 10
    /// mushrooms. Note that this is NOT the normal model id. it is the
    /// `model_id + (class as usize) * 1000` if I remember correctly. Pretty
    /// sure nobody will ever uses this though, as it is only for looks.
    ChangeItemLook {
        inv: ItemPlace,
        pos: usize,
        raw_model_id: u16,
    },
    /// Continues the expedition by picking one of the <=3 encounters \[0,1,2\]
    ExpeditionPickEncounter {
        /// The position of the encounter you want to pick
        pos: usize,
    },
    /// Continues the expedition, if you are currently in a situation, where
    /// there is only one option. This can be starting a fighting, or starting
    /// the wait after a fight (collecting the non item reward). Behind the
    /// scenes this is just ExpeditionPickReward(0)
    ExpeditionContinue,
    /// If there are multiple items to choose from after fighting a boss, you
    /// can choose which one to take here. \[0,1,2\]
    ExpeditionPickReward {
        /// The array position/index of the reward you want to take
        pos: usize,
    },
    /// Starts one of the two expeditions \[0,1\]
    ExpeditionStart {
        /// The index of the expedition to start
        pos: usize,
    },
    /// Skips the waiting period of the current expedition. Note that mushroom
    /// may not always be possible
    ExpeditionSkipWait {
        /// The "currency" you want to skip the expedition
        typ: TimeSkip,
    },
    /// This sets the "Questing instead of expeditions" value in the settings.
    /// This will decide if you can go on expeditions, or do quests, when
    /// expeditions are available. Going on the "wrong" one will return an
    /// error. Similarly this setting can only be changed, when no Thirst for
    /// Adventure has been used today, so make sure to check if that is full
    /// and `beer_drunk == 0`
    SetQuestsInsteadOfExpeditions {
        /// The value you want to set
        value: ExpeditionSetting,
    },
    HellevatorEnter,
    HellevatorViewGuildRanking,
    HellevatorFight {
        use_mushroom: bool,
    },
    HellevatorBuy {
        position: usize,
        typ: HellevatorTreatType,
        price: u32,
        use_mushroom: bool,
    },
    HellevatorRefreshShop,
    HellevatorJoinHellAttack {
        use_mushroom: bool,
        plain: usize,
    },
    HellevatorClaimDaily,
    HellevatorClaimFinal,
    HellevatorPreviewRewards,
    HallOfFameHellevatorPage {
        page: usize,
    },
    ClaimablePreview {
        msg_id: i64,
    },
    ClaimableClaim {
        msg_id: i64,
    },
    /// Spend 1000 mushrooms to buy a gold frame
    BuyGoldFrame,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// This is the "Questing instead of expeditions" value in the settings
pub enum ExpeditionSetting {
    /// When expeditions are available, this setting will enable expeditions to
    /// be started. This will disable questing, until either this setting is
    /// disabled, or expeditions have ended. Trying to start a quest with this
    /// setting set will return an error
    PreferExpeditions,
    /// When expeditions are available, they will be ignored, until either this
    /// setting is disabled, or expeditions have ended. Starting an
    /// expedition with this setting set will error
    #[default]
    PreferQuests,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlacksmithAction {
    Dismantle = 201,
    SocketUpgrade = 202,
    SocketUpgradeWithMushrooms = 212,
    GemExtract = 203,
    GemExtractWithMushrooms = 213,
    Upgrade = 204,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortunePayment {
    LuckyCoins = 0,
    Mushrooms,
    FreeTurn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The price you have to pay to roll the dice
pub enum RollDicePrice {
    Free = 0,
    Mushrooms,
    Hourglass,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of dice you want to play with.
pub enum DiceType {
    /// This means you want to discard whatever dice was previously at this
    /// position. This is also the type you want to fill the array with, if you
    /// start a game
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
    /// The resource you have won
    pub win_typ: DiceType,
    /// The amounts of the resource you have won
    pub amount: u32,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Enum, FromPrimitive, Hash, EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// A type of attribute
pub enum AttributeType {
    Strength = 1,
    Dexterity = 2,
    Intelligence = 3,
    Constitution = 4,
    Luck = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// A type of shop. This is a subset of `ItemPlace`
pub enum ShopType {
    Weapon = 3,
    Magic = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The "currency" you want to use to skip a quest
pub enum TimeSkip {
    Mushroom = 1,
    Glass = 2,
}

impl Command {
    /// Returns the unencrypted string, that has to be send to the server to to
    /// perform the request
    #[allow(deprecated, clippy::useless_format)]
    #[cfg(feature = "session")]
    pub(crate) fn request_string(
        &self,
    ) -> Result<String, crate::error::SFError> {
        const APP_VERSION: &str = "2100000000000";
        use crate::{
            error::SFError,
            gamestate::dungeons::{LightDungeon, ShadowDungeon},
            misc::{sha1_hash, to_sf_string, HASH_CONST},
        };

        Ok(match self {
            Command::Custom {
                cmd_name,
                arguments: values,
            } => {
                format!("{cmd_name}:{}", values.join("/"))
            }
            Command::Login {
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
            Command::SSOLogin {
                uuid, character_id, ..
            } => format!(
                "SFAccountCharLogin:{uuid}/{character_id}/unity3d_webglplayer/\
                 /{APP_VERSION}"
            ),
            Command::Register {
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
            Command::Update => "Poll:".to_string(),
            Command::HallOfFamePage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("PlayerGetHallOfFame:{pos}//25/25")
            }
            Command::HallOfFameFortressPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("FortressGetHallOfFame:{pos}//25/25")
            }
            Command::HallOfFameGroupPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("GroupGetHallOfFame:{pos}//25/25")
            }
            Command::HallOfFameUnderworldPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("UnderworldGetHallOfFame:{pos}//25/25")
            }
            Command::HallOfFamePetsPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("PetsGetHallOfFame:{pos}//25/25")
            }
            Command::ViewPlayer { ident } => format!("PlayerLookAt:{ident}"),
            Command::BuyBeer => format!("PlayerBeerBuy:"),
            Command::StartQuest {
                quest_pos,
                overwrite_inv,
            } => {
                format!(
                    "PlayerAdventureStart:{}/{}",
                    quest_pos + 1,
                    u8::from(*overwrite_inv)
                )
            }
            Command::CancelQuest => format!("PlayerAdventureStop:"),
            Command::FinishQuest { skip } => {
                format!(
                    "PlayerAdventureFinished:{}",
                    skip.map(|a| a as u8).unwrap_or(0)
                )
            }
            Command::StartWork { hours } => format!("PlayerWorkStart:{hours}"),
            Command::CancelWork => format!("PlayerWorkStop:"),
            Command::FinishWork => format!("PlayerWorkFinished:"),
            Command::CheckNameAvailable { name } => {
                format!("AccountCheck:{name}")
            }
            Command::BuyMount { mount } => {
                format!("PlayerMountBuy:{}", *mount as usize)
            }
            Command::IncreaseAttribute {
                attribute,
                increase_to,
            } => format!(
                "PlayerAttributIncrease:{}/{increase_to}",
                *attribute as u8
            ),
            Command::RemovePotion { pos } => {
                format!("PlayerPotionKill:{}", pos + 1)
            }
            Command::CheckArena => format!("PlayerArenaEnemy:"),
            Command::Fight { name, use_mushroom } => {
                format!("PlayerArenaFight:{name}/{}", u8::from(*use_mushroom))
            }
            Command::CollectCalendar => format!("PlayerOpenCalender:"),
            Command::UpgradeSkill {
                attribute,
                next_attribute,
            } => format!(
                "PlayerAttributIncrease:{}/{next_attribute}",
                *attribute as i64
            ),
            Command::RefreshShop { shop } => {
                format!("PlayerNewWares:{}", *shop as usize - 2)
            }
            Command::ViewGuild { guild_ident } => {
                format!("GroupLookAt:{guild_ident}")
            }
            Command::GuildFound { name } => format!("GroupFound:{name}"),
            Command::GuildInvitePlayer { name } => {
                format!("GroupInviteMember:{name}")
            }
            Command::GuildKickPlayer { name } => {
                format!("GroupRemoveMember:{name}")
            }
            Command::GuildSetLeader { name } => {
                format!("GroupSetLeader:{name}")
            }
            Command::GuildToggleOfficer { name } => {
                format!("GroupSetOfficer:{name}")
            }
            Command::GuildLoadMushrooms => {
                format!("GroupIncreaseBuilding:0")
            }
            Command::GuildIncreaseSkill { skill, current } => {
                format!("GroupSkillIncrease:{}/{current}", *skill as usize)
            }
            Command::GuildJoinAttack => format!("GroupReadyAttack:"),
            Command::GuildJoinDefense => format!("GroupReadyDefense:"),
            Command::GuildAttack { guild } => {
                format!("GroupAttackDeclare:{guild}")
            }
            Command::GuildRaid => format!("GroupRaidDeclare:"),
            Command::ToiletFlush => format!("PlayerToilettFlush:"),
            Command::ToiletOpen => format!("PlayerToilettOpenWithKey:"),
            Command::FightTower {
                current_level: progress,
                use_mush,
            } => {
                format!("PlayerTowerBattle:{progress}/{}", u8::from(*use_mush))
            }
            Command::ToiletDrop { inventory, pos } => {
                format!("PlayerToilettLoad:{}/{}", *inventory as usize, pos + 1)
            }
            Command::GuildPortalBattle => format!("GroupPortalBattle:"),
            Command::GuildGetFightableTargets => {
                format!("GroupFightableTargets:")
            }
            Command::FightPortal => format!("PlayerPortalBattle:"),
            Command::MessageOpen { pos: index } => {
                format!("PlayerMessageView:{}", *index + 1)
            }
            Command::MessageDelete { pos: index } => format!(
                "PlayerMessageDelete:{}",
                match index {
                    -1 => -1,
                    x => *x + 1,
                }
            ),
            Command::ViewScrapbook => format!("PlayerPollScrapbook:"),
            Command::ViewPet { pet_id: pet_index } => {
                format!("PetsGetStats:{pet_index}")
            }
            Command::BuyShop {
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
            Command::SellShop {
                inventory,
                inventory_pos,
            } => {
                let mut rng = fastrand::Rng::new();
                let shop = if rng.bool() {
                    ShopType::Magic
                } else {
                    ShopType::Weapon
                };
                let shop_pos = rng.u32(0..6);
                format!(
                    "PlayerItemMove:{}/{}/{}/{}",
                    *inventory as usize,
                    *inventory_pos + 1,
                    shop as usize,
                    shop_pos + 1,
                )
            }
            Command::InventoryMove {
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
            Command::ItemMove {
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
            Command::UsePotion { from, from_pos } => {
                format!(
                    "PlayerItemMove:{}/{}/1/0/",
                    *from as usize,
                    *from_pos + 1
                )
            }
            Command::UnlockFeature { unlockable } => format!(
                "UnlockFeature:{}/{}",
                unlockable.main_ident, unlockable.sub_ident
            ),
            Command::GuildSetInfo {
                description,
                emblem,
            } => format!(
                "GroupSetDescription:{}§{}",
                emblem.server_encode(),
                to_sf_string(description)
            ),
            Command::SetDescription { description } => {
                format!("PlayerSetDescription:{}", &to_sf_string(description))
            }
            Command::GuildSendChat { message } => {
                format!("GroupChat:{}", &to_sf_string(message))
            }
            Command::GambleSilver { amount } => {
                format!("PlayerGambleGold:{amount}")
            }
            Command::GambleMushrooms { amount } => {
                format!("PlayerGambleCoins:{amount}")
            }
            Command::SendMessage { to, msg } => {
                format!("PlayerMessageSend:{to}/{}", to_sf_string(msg))
            }
            Command::WitchDropCauldron {
                inventory_t,
                position,
            } => format!(
                "PlayerWitchSpendItem:{}/{}",
                *inventory_t as usize,
                position + 1
            ),
            Command::Blacksmith {
                inventory_t,
                position,
                action,
            } => format!(
                "PlayerItemMove:{}/{}/{}/-1",
                *inventory_t as usize,
                position + 1,
                *action as usize
            ),
            Command::WitchEnchant { enchantment } => {
                format!("PlayerWitchEnchantItem:{}/1", enchantment.enchant_id())
            }
            Command::SpinWheelOfFortune {
                payment: fortune_payment,
            } => {
                format!("WheelOfFortune:{}", *fortune_payment as usize)
            }
            Command::FortressGather { resource } => {
                format!("FortressGather:{}", *resource as usize + 1)
            }
            Command::EquipCompanion {
                from_inventory,
                from_pos,
                to_slot,
                to_companion,
            } => format!(
                "PlayerItemMove:{}/{}/{}/{}",
                *from_inventory as usize,
                *from_pos,
                *to_companion as u8 + 101,
                *to_slot as usize
            ),
            Command::FortressBuild { f_type } => {
                format!("FortressBuildStart:{}/0", *f_type as usize + 1)
            }
            Command::FortressBuildCancel { f_type } => {
                format!("FortressBuildStop:{}", *f_type as usize + 1)
            }
            Command::FortressBuildFinish { f_type, mushrooms } => format!(
                "FortressBuildFinished:{}/{mushrooms}",
                *f_type as usize + 1
            ),
            Command::FortressBuildUnit { unit, count } => {
                format!("FortressBuildUnitStart:{}/{count}", *unit as usize + 1)
            }
            Command::FortressGemStoneSearch => {
                format!("FortressGemstoneStart:",)
            }
            Command::FortressGemStoneSearchCancel => {
                format!("FortressGemStoneStop:0")
            }
            Command::FortressGemStoneSearchFinish { mushrooms } => {
                format!("FortressGemstoneFinished:{mushrooms}",)
            }
            Command::FortressAttack { soldiers } => {
                format!("FortressAttack:{soldiers}")
            }
            Command::FortressNewEnemy { use_mushroom: pay } => {
                format!("FortressEnemy:{}", usize::from(*pay))
            }
            Command::FortressSetCAEnemy { msg_id } => {
                format!("FortressEnemy:0/{}", *msg_id)
            }
            Command::FortressUpgradeHallOfKnights => {
                format!("FortressGroupBonusUpgrade:")
            }
            Command::Whisper {
                player_name: player,
                message,
            } => format!(
                "PlayerMessageWhisper:{}/{}",
                player,
                to_sf_string(message)
            ),
            Command::UnderworldCollect {
                resource: resource_t,
            } => {
                format!("UnderworldGather:{}", *resource_t as usize + 1)
            }
            Command::UnderworldUnitUpgrade { unit: unit_t } => {
                format!("UnderworldUpgradeUnit:{}", *unit_t as usize + 1)
            }
            Command::UnderworldUpgradeStart {
                building,
                mushrooms,
            } => format!(
                "UnderworldBuildStart:{}/{mushrooms}",
                *building as usize + 1
            ),
            Command::UnderworldUpgradeCancel { building } => {
                format!("UnderworldBuildStop:{}", *building as usize + 1)
            }
            Command::UnderworldUpgradeFinish {
                building,
                mushrooms,
            } => format!(
                "UnderworldBuildFinished:{}/{mushrooms}",
                *building as usize + 1
            ),
            Command::UnderworldAttack { player_id } => {
                format!("UnderworldAttack:{player_id}")
            }
            Command::RollDice { payment, dices } => {
                let mut dices = dices.iter().fold(String::new(), |mut a, b| {
                    if !a.is_empty() {
                        a.push('/');
                    }
                    a.push((*b as u8 + b'0') as char);
                    a
                });

                if dices.is_empty() {
                    // FIXME: This is dead code, right?
                    dices = "0/0/0/0/0".to_string();
                }
                format!("RollDice:{}/{}", *payment as usize, dices)
            }
            Command::PetFeed { pet_id, fruit_idx } => {
                format!("PlayerPetFeed:{pet_id}/{fruit_idx}")
            }
            Command::GuildPetBattle { use_mushroom } => {
                format!("GroupPetBattle:{}", usize::from(*use_mushroom))
            }
            Command::IdleUpgrade { typ: kind, amount } => {
                format!("IdleIncrease:{}/{}", *kind as usize, amount)
            }
            Command::IdleSacrifice => format!("IdlePrestige:0"),
            Command::SwapManequin => format!("PlayerDummySwap:301/1"),
            Command::UpdateFlag { flag } => format!(
                "PlayerSetFlag:{}",
                flag.map(Flag::code).unwrap_or_default()
            ),
            Command::BlockGuildInvites { block_invites } => {
                format!("PlayerSetNoGroupInvite:{}", u8::from(*block_invites))
            }
            Command::ShowTips { show_tips } => {
                #[allow(clippy::unreadable_literal)]
                {
                    format!(
                        "PlayerTutorialStatus:{}",
                        if *show_tips { 0 } else { 0xFFFFFFF }
                    )
                }
            }
            Command::ChangePassword { username, old, new } => {
                let old = sha1_hash(&format!("{old}{HASH_CONST}"));
                let new = sha1_hash(&format!("{new}{HASH_CONST}"));
                format!("AccountPasswordChange:{username}/{old}/106/{new}/")
            }
            Command::ChangeMailAddress {
                old_mail,
                new_mail,
                password,
                username,
            } => {
                let pass = sha1_hash(&format!("{password}{HASH_CONST}"));
                format!(
                    "AccountMailChange:{old_mail}/{new_mail}/{username}/\
                     {pass}/106"
                )
            }
            Command::SetLanguage { language } => {
                format!("AccountSetLanguage:{language}")
            }
            Command::SetPlayerRelation {
                player_id,
                relation,
            } => format!("PlayerFriendSet:{player_id}/{}", *relation as i32),
            Command::SetPortraitFrame { portrait_id } => {
                format!("PlayerSetActiveFrame:{portrait_id}")
            }
            Command::CollectDailyQuestReward { pos } => {
                format!("DailyTaskClaim:1/{}", pos + 1)
            }
            Command::CollectEventTaskReward { pos } => {
                format!("DailyTaskClaim:2/{}", pos + 1)
            }
            Command::SwapRunes {
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
            Command::ChangeItemLook {
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
            Command::ExpeditionPickEncounter { pos } => {
                format!("ExpeditionProceed:{}", pos + 1)
            }
            Command::ExpeditionContinue => format!("ExpeditionProceed:1"),
            Command::ExpeditionPickReward { pos } => {
                format!("ExpeditionProceed:{}", pos + 1)
            }
            Command::ExpeditionStart { pos } => {
                format!("ExpeditionStart:{}", pos + 1)
            }
            Command::FightDungeon {
                dungeon,
                use_mushroom,
            } => match dungeon {
                Dungeon::Light(name) => {
                    if *name == LightDungeon::Tower {
                        return Err(SFError::InvalidRequest(
                            "The tower must be fought with the FightTower \
                             command",
                        ));
                    }
                    format!(
                        "PlayerDungeonBattle:{}/{}",
                        *name as usize + 1,
                        u8::from(*use_mushroom)
                    )
                }
                Dungeon::Shadow(name) => {
                    if *name == ShadowDungeon::Twister {
                        format!(
                            "PlayerDungeonBattle:{}/{}",
                            LightDungeon::Tower as u32 + 1,
                            u8::from(*use_mushroom)
                        )
                    } else {
                        format!(
                            "PlayerShadowBattle:{}/{}",
                            *name as u32 + 1,
                            u8::from(*use_mushroom)
                        )
                    }
                }
            },
            Command::FightPetOpponent {
                opponent_id,
                habitat: element,
            } => {
                format!("PetsPvPFight:0/{opponent_id}/{}", *element as u32 + 1)
            }
            Command::FightPetDungeon {
                use_mush,
                habitat: element,
                enemy_pos,
                player_pet_id,
            } => {
                format!(
                    "PetsDungeonFight:{}/{}/{enemy_pos}/{player_pet_id}",
                    u8::from(*use_mush),
                    *element as u8 + 1,
                )
            }
            Command::ExpeditionSkipWait { typ } => {
                format!("ExpeditionTimeSkip:{}", *typ as u8)
            }
            Command::SetQuestsInsteadOfExpeditions { value } => {
                let value = match value {
                    ExpeditionSetting::PreferExpeditions => 'a',
                    ExpeditionSetting::PreferQuests => 'b',
                };
                format!("UserSettingsUpdate:5/{value}")
            }
            Command::HellevatorEnter => format!("GroupTournamentJoin:"),
            Command::HellevatorViewGuildRanking => {
                format!("GroupTournamentRankingOwnGroup")
            }
            Command::HellevatorFight { use_mushroom } => {
                format!("GroupTournamentBattle:{}", u8::from(*use_mushroom))
            }
            Command::HellevatorBuy {
                position,
                typ,
                price,
                use_mushroom,
            } => format!(
                "GroupTournamentMerchantBuy:{position}/{}/{price}/{}",
                *typ as u32,
                if *use_mushroom { 2 } else { 1 }
            ),
            Command::HellevatorRefreshShop => {
                format!("GroupTournamentMerchantReroll:")
            }
            Command::HallOfFameHellevatorPage { page } => {
                let per_page = 51;
                let pos = 26 + (per_page * page);
                format!("GroupTournamentRankingAllGroups:{pos}//25/25")
            }
            Command::HellevatorJoinHellAttack {
                use_mushroom,
                plain: pos,
            } => format!(
                "GroupTournamentRaidParticipant:{}/{}",
                u8::from(*use_mushroom),
                *pos + 1
            ),
            Command::HellevatorClaimDaily => {
                format!("GroupTournamentClaimDaily:")
            }
            Command::HellevatorPreviewRewards => {
                format!("GroupTournamentPreview:")
            }
            Command::HellevatorClaimFinal => format!("GroupTournamentClaim:"),
            Command::ClaimablePreview { msg_id } => {
                format!("PendingRewardView:{msg_id}")
            }
            Command::ClaimableClaim { msg_id } => {
                format!("PendingRewardClaim:{msg_id}")
            }
            Command::BuyGoldFrame => {
                format!("PlayerGoldFrameBuy:")
            }
        })
    }
}

macro_rules! generate_flag_enum {
    ($($variant:ident => $code:expr),*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[allow(missing_docs)]
        /// The flag of a country, that will be visible in the Hall of Fame
        pub enum Flag {
            $(
                $variant,
            )*
        }

        impl Flag {
            pub(crate) fn code(self) -> &'static str {
                match self {
                    $(
                        Flag::$variant => $code,
                    )*
                }
            }

            pub(crate) fn parse(value: &str) -> Option<Self> {
                if value.is_empty() {
                    return None;
                }

                // Mapping from string codes to enum variants
                match value {
                    $(
                        $code => Some(Flag::$variant),
                    )*

                    _ => {
                        warn!("Invalid flag value: {value}");
                        None
                    }
                }
            }
        }
    };
}

// Use the macro to generate the Flag enum and its methods
// Source: https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2#Officially_assigned_code_elements
generate_flag_enum! {
    Argentina => "ar",
    Australia => "au",
    Austria => "at",
    Belgium => "be",
    Bolivia => "bo",
    Brazil => "br",
    Bulgaria => "bg",
    Canada => "ca",
    Chile => "cl",
    China => "cn",
    Colombia => "co",
    CostaRica => "cr",
    Czechia => "cz",
    Denmark => "dk",
    DominicanRepublic => "do",
    Ecuador => "ec",
    ElSalvador =>"sv",
    Finland => "fi",
    France => "fr",
    Germany => "de",
    GreatBritain => "gb",
    Greece => "gr",
    Honduras => "hn",
    Hungary => "hu",
    India => "in",
    Italy => "it",
    Japan => "jp",
    Lithuania => "lt",
    Mexico => "mx",
    Netherlands => "nl",
    Panama => "pa",
    Paraguay => "py",
    Peru => "pe",
    Philippines => "ph",
    Poland => "pl",
    Portugal => "pt",
    Romania => "ro",
    Russia => "ru",
    SaudiArabia => "sa",
    Slovakia => "sk",
    SouthKorea => "kr",
    Spain => "es",
    Sweden => "se",
    Switzerland => "ch",
    Thailand => "th",
    Turkey => "tr",
    Ukraine => "ua",
    UnitedArabEmirates => "ae",
    UnitedStates => "us",
    Uruguay => "uy",
    Venezuela => "ve",
    Vietnam => "vn"
}
