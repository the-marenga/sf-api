//! Social interactions, mail, Hall of Fame rankings, and player/guild lookups.

/// Hall of Fame leaderboards for players, guilds, pets, fortresses, etc.
pub mod hall_of_fame;
/// Query cache and lookup storage for inspected players and guilds.
pub mod lookup;
/// Player mailbox, notifications, combat log, and claimable deliveries.
pub mod mail;
/// Inspected other guild profiles and member rosters.
pub mod other_guild;
/// Inspected other player profiles, equipment, and relation status.
pub mod other_player;

pub use hall_of_fame::*;
pub use lookup::*;
pub use mail::*;
pub use other_guild::*;
pub use other_player::*;
