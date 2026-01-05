use fastrand::Rng;

use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct NecromancerFightContext {
    data: FightContext,
    base_damage_multi: f64,
    minion_type: NecromancerMinionType,
    minion_rounds: i32,
    skeleton_revives: i32,
}

impl NecromancerFightContext {
    pub fn new(data: FightContext, base_damage_multi: f64) -> Self {
        Self {
            data,
            base_damage_multi,
            minion_type: NecromancerMinionType::None,
            minion_rounds: 0,
            skeleton_revives: 0,
        }
    }

    fn normal_attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        *round += 1;

        if self.minion_type == NecromancerMinionType::None
            && rng.i32(1..101) <= 50
        {
            self.summon_minion(rng);
            return self.attack_with_minion(target, round, rng);
        }

        if target.will_take_attack(rng) {
            let dmg = self.calculate_basic_hit_damage(*round, rng);
            if target.take_attack(dmg, round, rng) {
                return true;
            }
        }

        self.attack_with_minion(target, round, rng)
    }

    fn attack_with_minion(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        if (self.minion_type == NecromancerMinionType::None) {
            return false;
        }

        *round += 1;

        self.minion_rounds -= 1;
        let current_minion = self.minion_type;
        // Currently skeleton can revice once per fight but this is a bug
        if self.minion_rounds == 0
            && current_minion == NecromancerMinionType::Skeleton
            && self.skeleton_revives < 1
        {
            // FIXME: Why is this commented out?
            //&& Random.Next(1, 101) > 50)

            self.minion_rounds = 1;
            self.skeleton_revives += 1;
        } else if self.minion_rounds == 0 {
            self.minion_type = NecromancerMinionType::None;
            self.skeleton_revives = 0;
        }

        if !target.will_take_attack(rng) {
            return false;
        }

        let mut crit_chance = self.data.crit_chance;
        let mut crit_multi = self.data.crit_multiplier;
        if current_minion == NecromancerMinionType::Hound {
            crit_chance = (crit_chance + 0.1).min(0.6);
            crit_multi = 2.5 * (crit_multi / 2.0);
        }

        let mut dmg = self.calculate_hit_damage(
            &self.context().damage,
            *round,
            crit_chance,
            crit_multi,
            rng,
        );
        dmg *= self.get_minion_dmg_multiplier(current_minion);
        target.take_attack(dmg, round, rng)
    }

    fn summon_minion(&mut self, rng: &mut Rng) {
        let minion_type_chance = rng.i32(1..4);

        (self.minion_type, self.minion_rounds) = match minion_type_chance {
            1 => (NecromancerMinionType::Skeleton, 3),
            2 => (NecromancerMinionType::Hound, 2),
            _ => (NecromancerMinionType::Golem, 4),
        };
    }

    fn get_minion_dmg_multiplier(
        &self,
        minion_type: NecromancerMinionType,
    ) -> f64 {
        let base = self.base_damage_multi;
        match minion_type {
            NecromancerMinionType::Skeleton => (base + 0.25) / base,
            NecromancerMinionType::Hound => (base + 1.0) / base,
            NecromancerMinionType::Golem => 1.0,
            NecromancerMinionType::None => 0.0,
        }
    }
}

impl Fightable for NecromancerFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }

    fn reset_state(&mut self) {
        self.reset_health();
        self.minion_rounds = 0;
        self.skeleton_revives = 0;
        self.minion_type = NecromancerMinionType::None;
    }
    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        if self.data.opponent_is_mage {
            return true;
        }
        if self.minion_type != NecromancerMinionType::Golem {
            return true;
        }
        rng.i32(1..101) > 25
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
            return self.attack_generic(target, round, rng);
        }
        self.normal_attack(target, round, rng)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum NecromancerMinionType {
    None,
    Skeleton,
    Hound,
    Golem,
}
