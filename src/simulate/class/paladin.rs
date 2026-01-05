use fastrand::Rng;

use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct PaladinFightContext {
    data: FightContext,
    initial_armor_reduction: f64,
    stance: PaladinStanceType,
}

impl PaladinFightContext {
    pub fn new(data: FightContext, initial_armor_reduction: f64) -> Self {
        Self {
            data,
            initial_armor_reduction,
            stance: PaladinStanceType::Initial,
        }
    }
    fn change_stance(&mut self, rng: &mut Rng) {
        if rng.bool() {
            return;
        }
        self.stance = match self.stance {
            PaladinStanceType::Initial => PaladinStanceType::Defensive,
            PaladinStanceType::Defensive => PaladinStanceType::Offensive,
            PaladinStanceType::Offensive => PaladinStanceType::Initial,
        };
    }

    pub(crate) fn current_armor_reduction(&self) -> f64 {
        match self.stance {
            PaladinStanceType::Initial | PaladinStanceType::Defensive => 1.0,
            PaladinStanceType::Offensive => {
                1.0 / (1.0 - self.initial_armor_reduction)
                    * (1.0 - self.initial_armor_reduction.min(0.20))
            }
        }
    }

    fn attack_normal(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        *round += 1;
        self.change_stance(rng);

        if (!target.will_take_attack(rng)) {
            return false;
        }

        let dmg = self.calculate_basic_hit_damage(*round, rng)
            * self.stance.damage_multiplier();

        target.take_attack(dmg, round, rng)
    }
}

impl Fightable for PaladinFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }

    fn attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool
    where
        Self: std::marker::Sized,
    {
        if target.is_mage() {
            self.attack_generic(target, round, rng)
        } else {
            self.attack_normal(target, round, rng)
        }
    }

    fn reset_state(&mut self) {
        self.reset_health();
        self.stance = PaladinStanceType::Initial;
    }

    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        self.stance == PaladinStanceType::Defensive
            || rng.i32(1..101) > self.stance.block_chance()
    }

    fn take_attack(
        &mut self,
        damage: f64,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        if self.data.opponent_is_mage {
            let health = &mut self.data.fighter.health;
            *health -= damage;
            return *health <= 0.0;
        }
        let actual_damage = damage * self.current_armor_reduction();
        let current_health = &mut self.data.fighter.health;

        if (self.stance == PaladinStanceType::Defensive
            && rng.i32(1..101) <= self.stance.block_chance())
        {
            let health = actual_damage * 0.3;
            *current_health +=
                (self.data.max_health - *current_health).clamp(0.0, health);
            return false;
        }

        *current_health -= actual_damage;
        *current_health <= 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaladinStanceType {
    Initial,
    Defensive,
    Offensive,
}

impl PaladinStanceType {
    pub(crate) fn damage_multiplier(self) -> f64 {
        match self {
            PaladinStanceType::Initial => 1.0,
            PaladinStanceType::Defensive => 1.0 / 0.833 * 0.568,
            PaladinStanceType::Offensive => 1.0 / 0.833 * 1.253,
        }
    }

    pub(crate) fn block_chance(self) -> i32 {
        match self {
            PaladinStanceType::Initial => 30,
            PaladinStanceType::Defensive => 50,
            PaladinStanceType::Offensive => 25,
        }
    }
}
