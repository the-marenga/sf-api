use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;

use crate::{
    error::{Result, SFError},
    gamestate::{
        ServerTime,
        items::{ITEM_PARSE_LEN, Item},
        rewards::{Reward, RewardType},
    },
    response::{Vals, from_sf_string},
};

/// Information about the character's mailbox, combat log, and claimable
/// rewards.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mail {
    /// All the fights, that the character has stored for some reason
    pub combat_log: Vec<CombatLogEntry>,
    /// The amount of messages the inbox can store
    pub inbox_capacity: u16,
    /// Messages and notifications
    pub inbox: Vec<InboxEntry>,
    /// News from the server
    pub news_inbox: Vec<NewsEntry>,
    /// Items and resources from item codes/twitch drops, that you can claim
    pub claimables: Vec<ClaimableMail>,
    /// If you open a message, or news entry (via command), this here will
    /// contain the opened message
    pub open_msg: Option<String>,
    /// A preview of a claimable. You can get this via
    /// `Command::ClaimablePreview`
    pub open_claimable: Option<ClaimablePreview>,
}

impl Mail {
    /// Updates news entries from a `newslist` response.
    pub(crate) fn update_news_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.news_inbox.clear();
        for msg in data.iter().filter(|a| !a.trim().is_empty()) {
            match NewsEntry::parse(&Vals::by(msg, ','), server_time) {
                Ok(entry) => self.news_inbox.push(entry),
                Err(e) => warn!("Invalid news msg: {msg} {e}"),
            }
        }
    }

    /// Updates inbox messages from an `inbox` response.
    pub(crate) fn update_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.inbox.clear();
        for msg in data.iter().filter(|a| !a.trim().is_empty()) {
            match InboxEntry::parse(&Vals::by(msg, ','), server_time) {
                Ok(entry) => self.inbox.push(entry),
                Err(e) => warn!("Invalid inbox msg: {msg} {e}"),
            }
        }
    }

    /// Updates combat log entries from a `combatloglist` response.
    pub(crate) fn update_combat_log(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.combat_log.clear();
        for entry in data.iter() {
            if entry.is_empty() || entry.split(',').all(str::is_empty) {
                continue;
            }
            let entry_data = Vals::by(entry, ',');
            match CombatLogEntry::parse(&entry_data, server_time) {
                Ok(cle) => self.combat_log.push(cle),
                Err(e) => {
                    warn!("Unable to parse combat log entry: {entry} - {e}");
                }
            }
        }
    }

    /// Updates claimable deliveries from a `pendingrewards` response.
    pub(crate) fn update_claimables(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.claimables = data
            .chunks(6)
            .filter_map(|chunk| ClaimableMail::parse(&chunk, server_time).ok())
            .collect();
    }
}

/// The type of combat represented in a combat log entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CombatMessageType {
    /// An arena fight against another player
    Arena = 0,
    /// A tavern quest fight
    Quest = 1,
    /// A guild fight
    GuildFight = 2,
    /// A guild raid fight
    GuildRaid = 3,
    /// A dungeon fight
    Dungeon = 4,
    /// A tower fight
    TowerFight = 5,
    /// A lost fight
    LostFight = 6,
    /// A won fight
    WonFight = 7,
    /// An attack against a fortress
    FortressFight = 8,
    /// A fortress defense against an attacker
    FortressDefense = 9,
    /// A shadow world fight
    ShadowWorld = 12,
    /// A pet attack
    PetAttack = 14,
    /// A pet defense against an attacker
    PetDefense = 15,
    /// An underworld fight
    Underworld = 16,
    /// A twister fight
    Twister = 25,
    /// A lost guild fight
    GuildFightLost = 26,
    /// A won guild fight
    GuildFightWon = 27,
    /// A fortress defense where the attacker was already countered
    FortressDefenseAlreadyCountered = 109,
}

/// The type of an inbox message
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MessageType {
    /// A regular message
    #[default]
    Normal,
    /// An invitation to join a guild
    GuildInvite,
    /// A notification that the player was kicked from a guild
    GuildKicked,
}

/// An entry in the combat log
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CombatLogEntry {
    /// The unique message ID of the combat log entry
    pub msg_id: i64,
    /// The opponent's player or monster/guild name
    pub player_name: String,
    /// Whether the fight was won
    pub won: bool,
    /// The type of combat
    pub battle_type: CombatMessageType,
    /// The local timestamp when the battle occurred
    pub time: DateTime<Local>,
}

impl CombatLogEntry {
    /// Parses a combat log entry from a comma-separated record:
    /// `msg_id, player_name, won, battle_type, time, ...`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<CombatLogEntry> {
        let battle_type = data.parse_enum(3)?;
        let time = data.server_time(4, server_time)?.ok_or_else(|| {
            SFError::ParsingError("missing combat time".to_string())
        })?;

        if data.len() > 5 {
            data.ignore(5, "unknown combat log field");
        }

        Ok(CombatLogEntry {
            msg_id: data.i64(0)?,
            player_name: data.str(1)?.to_string(),
            won: data.str(2)? == "1",
            battle_type,
            time,
        })
    }
}

/// An entry in the player's message inbox
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InboxEntry {
    /// The message type
    pub msg_typ: MessageType,
    /// The sender's name
    pub from: String,
    /// The unique message ID
    pub msg_id: i32,
    /// The subject or title of the message
    pub title: String,
    /// The date and time the message was received
    pub date: DateTime<Local>,
    /// Whether the message has been marked as read
    pub read: bool,
}

impl InboxEntry {
    /// Parses an inbox entry from a comma-separated message string.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<InboxEntry> {
        if data.len() < 5 {
            return Err(SFError::ParsingError(format!(
                "Inbox entry too short (len: {})",
                data.len()
            )));
        }

        let last_idx = data.len() - 1;
        let time =
            data.server_time(last_idx, server_time)?.ok_or_else(|| {
                SFError::ParsingError("missing msg date".to_string())
            })?;

        let title_parts = data.slice(3..last_idx);
        let title_raw = title_parts.join(",");

        let msg_typ = match title_raw.as_str() {
            "3" => MessageType::GuildKicked,
            "5" => MessageType::GuildInvite,
            x if !x.is_empty() && x.chars().all(|a| a.is_ascii_digit()) => {
                return Err(SFError::ParsingError(format!(
                    "Unknown inbox msg type: {title_raw}"
                )));
            }
            _ => MessageType::Normal,
        };

        Ok(InboxEntry {
            msg_typ,
            date: time,
            from: data.str(1)?.to_string(),
            msg_id: data.i32(0)?,
            title: from_sf_string(title_raw.trim_end_matches('\t')),
            read: data.str(2)? != "0",
        })
    }
}

/// A news entry distributed by the server
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewsEntry {
    /// The id used to read the full contents of this message
    pub news_id: i64,
    /// The title of this news entry
    pub title: String,
    /// The time at which this news entry was distributed to players
    pub date: DateTime<Local>,
    /// Has this news entry been opened before?
    pub read: bool,
}

impl NewsEntry {
    /// Parses a news entry from a comma-separated record:
    /// `read, title, timestamp`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<NewsEntry> {
        let ts = data.i64(2)?;
        let time = data.server_time(2, server_time)?.ok_or_else(|| {
            SFError::ParsingError(format!("missing news date: {ts}"))
        })?;

        if data.len() > 3 {
            data.ignore(3, "unused news icon");
        }

        Ok(NewsEntry {
            news_id: ts,
            date: time,
            title: from_sf_string(data.str(1)?),
            read: data.str(0)? == "1",
        })
    }
}

/// A claimable mail delivery (e.g., promo codes, Twitch drops)
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimableMail {
    /// The message identifier
    pub msg_id: i64,
    /// The type of claimable mail
    pub typ: ClaimableMailType,
    /// The status of the claimable item
    pub status: ClaimableStatus,
    /// The name or subject of the claimable delivery
    pub name: String,
    /// The timestamp when the delivery was received
    pub received: Option<DateTime<Local>>,
    /// The expiration time until which the item can be claimed
    pub claimable_until: Option<DateTime<Local>>,
}

/// The claim status of a claimable mail delivery
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableStatus {
    /// The mail has not been viewed yet
    #[default]
    Unread,
    /// The mail has been viewed but not yet claimed
    Read,
    /// The rewards have already been claimed
    Claimed,
}

/// The type of a claimable mail
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableMailType {
    /// A coupon or promotional code reward
    Coupon = 10,
    /// A flying courier / superman delivery
    SupermanDelivery = 11,
    /// A Twitch drop delivery
    TwitchDrop = 12,
    /// A generic delivery
    #[default]
    GenericDelivery,
}

/// A preview of the contents of a claimable mail
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimablePreview {
    /// The items contained in the preview
    pub items: Vec<Item>,
    /// The resources or rewards contained in the preview
    pub resources: Vec<Reward>,
}

impl ClaimableMail {
    /// Parses a claimable mail delivery from a 6-element response slice:
    /// `[msg_id, status, type, code_name, start_time, end_time]`.
    ///
    /// # Errors
    /// Returns an error if required fields are missing or invalid.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<Self> {
        let status = match data.get_str(1) {
            Some("0") => ClaimableStatus::Unread,
            Some("1") => ClaimableStatus::Read,
            Some("2") => ClaimableStatus::Claimed,
            Some(x) => {
                warn!("Unknown claimable status: {x}");
                ClaimableStatus::Claimed
            }
            None => {
                return Err(SFError::ParsingError(
                    "Missing claimable status".to_string(),
                ));
            }
        };

        Ok(Self {
            msg_id: data.i64(0)?,
            status,
            typ: data.enum_or(2, ClaimableMailType::GenericDelivery),
            name: data.get(3)?.to_string(),
            received: data.server_time(4, server_time)?,
            claimable_until: data.server_time(5, server_time)?,
        })
    }
}

impl ClaimablePreview {
    /// Updates the resources in this preview from a `pendingrewardressources`
    /// response.
    pub(crate) fn update_resources(&mut self, data: &Vals<'_>) {
        self.resources = data
            .chunks(2)
            .map(|chunk| Reward {
                typ: RewardType::parse(chunk.i64_or(0, 0)),
                amount: chunk.u64_or(1, 1),
            })
            .collect();
    }

    /// Updates the items in this preview from a `pendingreward` response.
    pub(crate) fn update_items(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.items = data
            .chunks(ITEM_PARSE_LEN)
            .filter_map(|chunk| Item::parse(&chunk, server_time).ok().flatten())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_log_entry_parse() {
        let raw = "94224194,OpponentPlayer,1,0,1600000000";
        let data = Vals::by(raw, ',');
        let entry =
            CombatLogEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(entry.msg_id, 94_224_194);
        assert_eq!(entry.player_name, "OpponentPlayer");
        assert!(entry.won);
        assert_eq!(entry.battle_type, CombatMessageType::Arena);
        data.assert_all_read();

        let raw_6 = "76078261,Geli123,0,9,1791901430,0";
        let data_6 = Vals::by(raw_6, ',');
        let entry_6 =
            CombatLogEntry::parse(&data_6, ServerTime::default()).unwrap();
        assert_eq!(entry_6.msg_id, 76_078_261);
        assert_eq!(entry_6.player_name, "Geli123");
        assert!(!entry_6.won);
        data_6.assert_all_read();
    }

    #[test]
    fn test_inbox_entry_parse() {
        let raw = "12345,Admin,0,Important News,1600000000";
        let data = Vals::by(raw, ',');
        let entry = InboxEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(entry.msg_id, 12345);
        assert_eq!(entry.from, "Admin");
        assert!(!entry.read);
        assert_eq!(entry.title, "Important News");
        assert_eq!(entry.msg_typ, MessageType::Normal);
        data.assert_all_read();

        let raw_invite = "12346,GuildLeader,1,5,1600000000";
        let data_invite = Vals::by(raw_invite, ',');
        let invite =
            InboxEntry::parse(&data_invite, ServerTime::default()).unwrap();
        assert_eq!(invite.msg_typ, MessageType::GuildInvite);

        let raw_commas = "12347,Friend,0,Hello,world!,1600000000";
        let data_commas = Vals::by(raw_commas, ',');
        let with_commas =
            InboxEntry::parse(&data_commas, ServerTime::default()).unwrap();
        assert_eq!(with_commas.title, "Hello,world!");
        data_commas.assert_all_read();
    }

    #[test]
    fn test_news_entry_parse() {
        let raw = "1,Update Announcement,1600000000,0";
        let data = Vals::by(raw, ',');
        let news = NewsEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(news.news_id, 1_600_000_000);
        assert_eq!(news.title, "Update Announcement");
        assert!(news.read);
        data.assert_all_read();
    }

    #[test]
    fn test_claimable_mail_parse() {
        let raw = "101/0/10/TESTCODE/1600000000/1700000000";
        let data = Vals::from(raw);
        let claimable =
            ClaimableMail::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(claimable.msg_id, 101);
        assert_eq!(claimable.status, ClaimableStatus::Unread);
        assert_eq!(claimable.typ, ClaimableMailType::Coupon);
        assert_eq!(claimable.name, "TESTCODE");
        assert!(claimable.received.is_some());
        assert!(claimable.claimable_until.is_some());
        data.assert_all_read();

        let mut mail = Mail::default();
        let list_raw = "101/0/10/TESTCODE/1600000000/1700000000/102/1/11/HERO/\
                        1600000000/1700000000";
        let list_data = Vals::from(list_raw);
        mail.update_claimables(&list_data, ServerTime::default());
        assert_eq!(mail.claimables.len(), 2);
        assert_eq!(mail.claimables[1].msg_id, 102);
        assert_eq!(mail.claimables[1].status, ClaimableStatus::Read);
        assert_eq!(mail.claimables[1].typ, ClaimableMailType::SupermanDelivery);
    }

    #[test]
    fn test_claimable_preview_update() {
        let mut preview = ClaimablePreview::default();
        let res_raw = "3/50/4/1000";
        let res_data = Vals::from(res_raw);
        preview.update_resources(&res_data);
        assert_eq!(preview.resources.len(), 2);
        assert_eq!(preview.resources[0].typ, RewardType::Mushrooms);
        assert_eq!(preview.resources[0].amount, 50);
        assert_eq!(preview.resources[1].typ, RewardType::Silver);
        assert_eq!(preview.resources[1].amount, 1000);
        res_data.assert_all_read();
    }

    #[test]
    fn test_mail_updates() {
        let mut mail = Mail::default();
        let news_raw = "1,News One,1600000000,0;2,News Two,1600000010,1";
        let news_data = Vals::by(news_raw, ';');
        mail.update_news_inbox(&news_data, ServerTime::default());
        assert_eq!(mail.news_inbox.len(), 2);
        news_data.assert_all_read();

        let inbox_raw = "10,Sender,0,Title,1600000000";
        let inbox_data = Vals::by(inbox_raw, ';');
        mail.update_inbox(&inbox_data, ServerTime::default());
        assert_eq!(mail.inbox.len(), 1);
        assert_eq!(mail.inbox[0].from, "Sender");
        inbox_data.assert_all_read();

        let combat_raw = "1001,OpponentPlayer,1,0,1600000000";
        let combat_data = Vals::by(combat_raw, ';');
        mail.update_combat_log(&combat_data, ServerTime::default());
        assert_eq!(mail.combat_log.len(), 1);
        assert_eq!(mail.combat_log[0].player_name, "OpponentPlayer");
        combat_data.assert_all_read();
    }
}
