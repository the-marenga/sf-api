use std::collections::HashMap;

use log::warn;

use super::{OtherGuild, OtherPlayer};
use crate::PlayerId;

/// Contains the results of `ViewGuild` & `ViewPlayer` commands. You can access
/// the player info via functions and the guild data directly
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lookup {
    /// This can be accessed by using the `lookup_pid()`/`lookup_name()`
    /// methods on `Lookup`
    players: HashMap<PlayerId, OtherPlayer>,
    name_to_id: HashMap<String, PlayerId>,

    /// Guild that the character has looked at
    pub guilds: HashMap<String, OtherGuild>,
}

impl Lookup {
    pub(crate) fn insert_lookup(&mut self, other: OtherPlayer) {
        if other.name.is_empty() || other.player_id == 0 {
            warn!("Skipping invalid player insert");
            return;
        }
        self.name_to_id.insert(other.name.clone(), other.player_id);
        self.players.insert(other.player_id, other);
    }

    /// Checks to see if we have queried a player with that player id
    #[must_use]
    pub fn lookup_pid(&self, pid: PlayerId) -> Option<&OtherPlayer> {
        self.players.get(&pid)
    }

    /// Checks to see if we have queried a player with the given name
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<&OtherPlayer> {
        let other_pos = self.name_to_id.get(name)?;
        self.players.get(other_pos)
    }

    /// Removes the information about another player based on their id
    #[allow(clippy::must_use_unit)]
    pub fn remove_pid(&mut self, pid: PlayerId) -> Option<OtherPlayer> {
        self.players.remove(&pid)
    }

    /// Removes the information about another player based on their name
    #[allow(clippy::must_use_unit)]
    pub fn remove_name(&mut self, name: &str) -> Option<OtherPlayer> {
        let other_pos = self.name_to_id.remove(name)?;
        self.players.remove(&other_pos)
    }

    /// Clears out all players, that have previously been queried
    pub fn reset_lookups(&mut self) {
        self.players = HashMap::default();
        self.name_to_id = HashMap::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_insert_and_retrieve() {
        let mut lookup = Lookup::default();
        let player = OtherPlayer {
            player_id: 12345,
            name: "Hero".to_string(),
            ..Default::default()
        };
        lookup.insert_lookup(player);
        assert!(lookup.lookup_pid(12345).is_some());
        assert_eq!(
            lookup.lookup_name("Hero").map(|p| p.player_id),
            Some(12345)
        );

        lookup.remove_name("Hero");
        assert!(lookup.lookup_pid(12345).is_none());
    }
}
