use std::collections::HashSet;

use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use crate::{error::Result, gamestate::ServerTime, response::Vals};

/// The type of event, that is currently happening on the server
#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    /// Exceptional XP event (+100% XP from quests).
    ExceptionalXPEvent = 0,
    /// Glorious Gold Galore event (extra gold).
    GloriousGoldGalore,
    /// Tidy Toilet Time event (mana from toilet).
    TidyToiletTime,
    /// Assembly of Awesome Animals event (pet food/find chance).
    AssemblyOfAwesomeAnimals,
    /// Fantastic Fortress Festivity event (faster fortress production/search).
    FantasticFortressFestivity,
    /// Days of Doomed Souls event (extra souls).
    DaysOfDoomedSouls,
    /// Witches' Dance event (witch cauldron boosts).
    WitchesDance,
    /// Sands of Time Special event (more hourglasses).
    SandsOfTimeSpecial,
    /// Forge Frenzy Festival event (blacksmith resources).
    ForgeFrenzyFestival,
    /// Epic Shopping Spree Extravaganza event (increased epic chance in shops).
    EpicShoppingSpreeExtravaganza,
    /// Epic Quest Extravaganza event (epics from quests).
    EpicQuestExtravaganza,
    /// Epic Good Luck Extravaganza event (wheel dice lucky rewards).
    EpicGoodLuckExtravaganza,
    /// One Beer, Two Beer, Free Beer event (free tavern beer).
    OneBeerTwoBeerFreeBeer,
    /// Piecework Party event (quick work).
    PieceworkParty,
    /// Lucky Day event (lucky coins and wheel spins).
    LuckyDay,
    /// Crazy Mushroom Harvest event (20% more mushrooms).
    CrazyMushroomHarvest,
    /// Holiday sale event.
    HolidaySale,
    /// Valentine's blessing event.
    ValentinesBlessing,
    /// Black Gem Rush event (higher black gem chance).
    BlackGemRush,
    /// Rumble for Riches event.
    RumbleForRiches,
}

/// Information about the events active in the tavern
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Events {
    /// All of the events active in the tavern
    pub active: HashSet<Event>,
    /// The time at which all of the events end. Mostly just Sunday 23:59.
    pub ends: Option<DateTime<Local>>,
}

impl Events {
    /// Updates the active events and their end time from an `events` response:
    /// `theme / flags / theme_end / ? / event_end / ...`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        if data.len() < 8 {
            data.ignore_all("too short events response");
            return Ok(());
        }
        data.ignore(0, "event theme / tavern decoration");
        self.active.clear();
        let flags = data.u64(1)?;
        for (idx, event) in Event::iter().enumerate() {
            if (flags & (1_u64 << idx)) != 0 {
                self.active.insert(event);
            }
        }
        data.ignore(2, "theme end timestamp");
        data.ignore(3, "unknown flag");
        self.ends = data.server_time(4, server_time)?;
        data.ignore(5..11, "trailing event values");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_events_update() {
        let raw = "52/2097664/1792108740/0/1792108740/0/0/0/0";
        let mut events = Events::default();
        let data = Vals::from(raw);
        events.update(&data, ServerTime::default()).unwrap();
        assert!(events.ends.is_some());
        assert!(!events.active.is_empty());
        data.assert_all_read();
    }
}
