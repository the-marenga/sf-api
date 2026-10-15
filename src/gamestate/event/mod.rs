//! Special and recurring event tracking.
//!
//! Contains generic event scheduling status
//! ([`EventStatus`](crate::gamestate::event::EventStatus)) and submodules
//! for individual events:
//! - [`hellevator`]
//! - [`legendary_dungeon`]
//! - [`world_boss`]

/// Hellevator event module
pub mod hellevator;
/// Legendary Dungeon event module
pub mod legendary_dungeon;
/// World Boss event module
pub mod world_boss;

use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use strum::EnumIter;

use crate::{
    gamestate::{Result, ServerTime},
    response::Vals,
};

/// Information about an upcoming or currently running special event.
///
/// Corresponds to the event banner/button above the Tavern in the official GUI.
/// Contains overall scheduling times for events like
/// [`Hellevator`](crate::gamestate::event::hellevator::HellevatorEvent),
/// [`Legendary
/// Dungeon`](crate::gamestate::event::legendary_dungeon::LegendaryDungeonEvent),
/// and [`World Boss`](crate::gamestate::event::world_boss::WorldBossEvent).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventStatus {
    /// The type of the special event
    #[doc(alias = "event_type")]
    pub typ: SpecialEventType,
    /// The time at which this event is scheduled to start
    pub start: Option<DateTime<Local>>,
    /// The time until which the event will be active
    pub end: Option<DateTime<Local>>,
    /// The deadline for post-event reward collection or claiming periods, if
    /// applicable
    #[doc(alias = "claim_end")]
    #[doc(alias = "collect_end")]
    pub extra_end: Option<DateTime<Local>>,
}

/// The specific kind of special event currently active or scheduled
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SpecialEventType {
    /// A legendary dungeon event with the given theme
    LegendaryDungeon(LegendaryDungeonTheme),
    /// The hellevator event
    Hellevator,
    /// Driving dungeon event
    DrivingDungeon,
    /// Traveling circus event
    TravelingCircus,
    /// World boss event with the given theme
    WorldBoss(WorldBossTheme),
    /// An unknown or unhandled event type
    #[default]
    Unknown,
}

/// Theme variants for the Legendary Dungeon event
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LegendaryDungeonTheme {
    /// Diabolical Company Party theme
    DiabolicalCompanyParty = 1,
    /// Lord of the Things theme
    LordOfTheThings,
    /// Fantastic Legendaries theme
    FantasticLegendaries,
    /// Shady Birthday Bash theme
    ShadyBirthdayBash,
    /// Winter Spectacle theme
    WinterSpectacle,
    /// Abysses of Madness theme
    AbyssesOfMadness,
    /// Hunt for the Easter Egg theme
    HuntForTheEasterEgg,
    /// Vile Vacation theme
    VileVacation,
    /// An unknown legendary dungeon theme
    #[default]
    Unknown,
}

/// Theme variants for the World Boss event
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossTheme {
    /// Normal world boss theme
    Normal = 1,
    /// Winter world boss theme
    Winter,
    /// Summer world boss theme
    Summer,
    /// An unknown world boss theme
    #[default]
    Unknown,
}

impl EventStatus {
    /// Parses an event status from a 5-value response:
    /// `[event_main_type, theme, start, end, claim_end]`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<Self> {
        let typ = match data.i64(0)? {
            1 => SpecialEventType::LegendaryDungeon(
                data.enum_opt(1).unwrap_or_default(),
            ),
            2 => {
                data.ignore(1, "unused theme for hellevator");
                SpecialEventType::Hellevator
            }
            3 => {
                data.ignore(1, "unused theme for driving dungeon");
                SpecialEventType::DrivingDungeon
            }
            4 => {
                data.ignore(1, "unused theme for traveling circus");
                SpecialEventType::TravelingCircus
            }
            6 => SpecialEventType::WorldBoss(
                data.enum_opt(1).unwrap_or_default(),
            ),
            _ => {
                data.ignore(1, "unused theme for unknown event");
                SpecialEventType::Unknown
            }
        };

        let start = data.server_time(2, server_time)?;
        let end = data.server_time(3, server_time)?;
        let extra_end = data.server_time(4, server_time)?;

        Ok(Self {
            typ,
            start,
            end,
            extra_end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_event_status_parse_legendary_dungeon() {
        let raw = "1/2/1784829600/1785096000/1785700800";
        let data = Vals::from(raw);
        let status = EventStatus::parse(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

        assert_eq!(
            status.typ,
            SpecialEventType::LegendaryDungeon(
                LegendaryDungeonTheme::LordOfTheThings
            )
        );
        assert!(status.start.is_some());
        assert!(status.end.is_some());
        assert!(status.extra_end.is_some());
    }

    #[test]
    fn test_event_status_parse_world_boss() {
        let raw = "6/1/1784829600/1785096000/1785700800";
        let data = Vals::from(raw);
        let status = EventStatus::parse(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

        assert_eq!(
            status.typ,
            SpecialEventType::WorldBoss(WorldBossTheme::Normal)
        );
        assert!(status.start.is_some());
        assert!(status.end.is_some());
        assert!(status.extra_end.is_some());
    }

    #[test]
    fn test_event_status_parse_hellevator() {
        let raw = "2/0/1784829600/1785096000/1785700800";
        let data = Vals::from(raw);
        let status = EventStatus::parse(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

        assert_eq!(status.typ, SpecialEventType::Hellevator);
        assert!(status.start.is_some());
        assert!(status.end.is_some());
        assert!(status.extra_end.is_some());
    }
}
