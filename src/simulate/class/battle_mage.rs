use crate::simulate::{Fightable, class::FightContext};

pub struct BattleMageFightContext {
    data: FightContext,
    fireball_dmg: f64,
}

impl BattleMageFightContext {
    pub fn new(data: FightContext, fireball_dmg: f64) -> Self {
        Self { data, fireball_dmg }
    }
}

impl Fightable for BattleMageFightContext {
    fn attack_before_fight(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        *round += 1;
        target.take_attack(self.fireball_dmg, round, rng)
    }

    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
}
