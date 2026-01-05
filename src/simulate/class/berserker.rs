use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct BerserkerFightContext {
    data: FightContext,
    chain_attack_counter: u32,
}

impl BerserkerFightContext {
    pub fn new(data: FightContext) -> Self {
        Self {
            data,
            chain_attack_counter: 0,
        }
    }
}

impl Fightable for BerserkerFightContext {
    fn reset_state(&mut self) {
        self.reset_health();
        self.chain_attack_counter = 0;
    }

    fn will_skip_round(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        if target.context().fighter.class == Class::Mage {
            return false;
        }

        if self.chain_attack_counter >= 14 {
            self.chain_attack_counter = 0;
        } else if rng.u32(1..=100) > 50 {
            *round += 1;
            self.chain_attack_counter += 1;
            return true;
        } else {
            self.chain_attack_counter = 0;
        }

        false
    }

    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
}
