use fastrand::Rng;

use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct PlagueDoctorFightContext {
    data: FightContext,
    base_damage_multiplier: f64,
    poison_round: i32,
    poison_dmg_multipliers: [f64; 3],
}

impl PlagueDoctorFightContext {
    pub fn new(data: FightContext, base_dmg_multi: f64) -> Self {
        let dmg_multiplier = Class::PlagueDoctor.get_config().damage_multiplier;
        let class_dmg_multi = base_dmg_multi / dmg_multiplier;

        let poison_multies = [
            (base_dmg_multi - 0.9 * class_dmg_multi) / base_dmg_multi,
            (base_dmg_multi - 0.55 * class_dmg_multi) / base_dmg_multi,
            (base_dmg_multi - 0.2 * class_dmg_multi) / base_dmg_multi,
        ];

        Self {
            data,
            base_damage_multiplier: base_dmg_multi,
            poison_round: 0,
            poison_dmg_multipliers: poison_multies,
        }
    }

    fn evade_chance(&self) -> i32 {
        match self.poison_round {
            3 => 65,
            2 => 50,
            1 => 35,
            _ => 20,
        }
    }
}

impl Fightable for PlagueDoctorFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }

    fn reset_state(&mut self) {
        self.poison_round = 0;
        self.reset_health();
    }

    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        rng.i32(1..101) > self.evade_chance()
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

        if self.poison_round <= 0 && rng.bool() {
            *round += 1;
            if (!target.will_take_attack(rng)) {
                return false;
            }

            self.poison_round = 3;
            let poison_multiplier = self.poison_dmg_multipliers[2];
            let tincture_throw_dmg = self.calculate_hit_damage(
                &(self.data.damage * poison_multiplier),
                *round,
                self.data.crit_chance,
                self.data.crit_multiplier,
                rng,
            );

            return target.take_attack(tincture_throw_dmg, round, rng);
        }

        if self.poison_round > 0 {
            *round += 1;
            self.poison_round -= 1;
            let poison_multiplier =
                self.poison_dmg_multipliers[self.poison_round as usize];
            let poison_dmg = self.calculate_hit_damage(
                &(self.data.damage * poison_multiplier),
                *round,
                self.data.crit_chance,
                self.data.crit_multiplier,
                rng,
            );

            if (target.take_attack(poison_dmg, round, rng)) {
                return true;
            }
        }
        self.attack_generic(target, round, rng)
    }
}
