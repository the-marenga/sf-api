use crate::simulate::{Fightable, class::FightContext, damage::DamageRange};

pub struct AssassinFightContext {
    context: FightContext,
    secondary_damage: DamageRange,
}

impl AssassinFightContext {
    pub fn new(context: FightContext, secondary_damage: DamageRange) -> Self {
        AssassinFightContext {
            context,
            secondary_damage,
        }
    }
}

impl Fightable for AssassinFightContext {
    fn context(&self) -> &FightContext {
        &self.context
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.context
    }

    fn attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        *round += 1;
        if target.will_take_attack(rng) {
            let first_weapon_damage =
                self.calculate_basic_hit_damage(*round, rng);
            if target.take_attack(first_weapon_damage, round, rng) {
                return true;
            }
        }

        *round += 1;

        if !target.will_take_attack(rng) {
            return false;
        }

        let second_weapon_damage = self.calculate_hit_damage(
            &self.context().damage,
            *round,
            self.context.crit_chance,
            self.context.crit_multiplier,
            rng,
        );

        target.take_attack(second_weapon_damage, round, rng)
    }

    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        rng.u32(1..=100) > 50
    }
}
