use crate::{
    gamestate::{character::Class, social::ClaimableMail},
    simulate::{Fightable, class::FightContext},
};

pub struct DemonHunterFightContext {
    data: FightContext,
    revive_count: u8,
}

impl DemonHunterFightContext {
    pub fn new(data: FightContext) -> Self {
        Self {
            data,
            revive_count: 0,
        }
    }

    fn revive(&mut self, round: &mut u32, rng: &mut fastrand::Rng) -> bool {
        let revive_chance = 0.44 - f64::from(self.revive_count) * 0.11;
        if revive_chance <= 0.0 || rng.f64() >= revive_chance {
            return true;
        }

        *round += 1;

        self.data.fighter.health =
            self.data.max_health * (0.9 - f64::from(self.revive_count) * 0.1);
        false
    }
}

impl Fightable for DemonHunterFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }

    fn reset_state(&mut self) {
        self.reset_health();
        self.revive_count = 0;
    }

    fn take_attack(
        &mut self,
        damage: f64,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        let health = &mut self.data.fighter.health;
        *health -= damage;
        if *health > 0.0 {
            return false;
        }
        if self.data.opponent_is_mage {
            return true;
        }
        self.revive(round, rng)
    }
}
