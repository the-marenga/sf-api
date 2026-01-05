use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct DruidFightContext {
    data: FightContext,
    evade_chance: i32,
    is_in_bear_form: bool,
    has_just_dodged: bool,
    rage_crit_chance: f64,
    rage_crit_multiplier_bonus: f64,
    swoop_chance: f64,
    swoop_damage_modifier: f64,
}

impl DruidFightContext {
    pub fn new(data: FightContext, rage_crit_chance: f64) -> Self {
        Self {
            data,
            evade_chance: 35,
            is_in_bear_form: false,
            rage_crit_chance,
            rage_crit_multiplier_bonus: 40.0,
            swoop_chance: 0.15,
            swoop_damage_modifier: (1.0 / 3.0 + 0.8) / (1.0 / 3.0),
            has_just_dodged: false,
        }
    }

    fn attack_bear_form(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        self.is_in_bear_form = true;
        self.has_just_dodged = false;
        *round += 1;

        if !target.will_take_attack(rng) {
            return false;
        }

        let crit_multiplier = (2.0 + self.rage_crit_multiplier_bonus)
            * self.context().crit_chance
            / 2.0;
        let dmg = self.calculate_hit_damage(
            &self.context().damage,
            *round,
            self.rage_crit_chance,
            crit_multiplier,
            rng,
        );
        target.take_attack(dmg, round, rng)
    }

    fn attack_not_bear_form(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        self.is_in_bear_form = false;
        let will_swoop = rng.f64() < self.swoop_chance;

        if will_swoop {
            *round += 1;
            self.swoop_chance = 0.5f64.min(self.swoop_chance + 0.05);

            if target.will_take_attack(rng) {
                let swoop_dmg = self.calculate_basic_hit_damage(*round, rng)
                    * self.swoop_damage_modifier;

                if target.take_attack(swoop_dmg, round, rng) {
                    return true;
                }
            }
        }

        *round += 1;
        if !target.will_take_attack(rng) {
            return false;
        }

        let dmg = self.calculate_basic_hit_damage(*round, rng);
        target.take_attack(dmg, round, rng)
    }
}

impl Fightable for DruidFightContext {
    fn reset_state(&mut self) {
        self.reset_health();
        self.swoop_chance = 0.15;
        self.is_in_bear_form = false;
        self.has_just_dodged = false;
    }

    fn attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        if target.is_mage() {
            return self.attack_generic(target, round, rng);
        }
        if self.has_just_dodged {
            return self.attack_bear_form(target, round, rng);
        }
        self.attack_not_bear_form(target, round, rng)
    }

    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        if !self.is_in_bear_form && rng.i32(1..101) <= self.evade_chance {
            self.has_just_dodged = true;
            return false;
        }
        true
    }

    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
}
