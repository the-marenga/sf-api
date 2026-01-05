use crate::{
    gamestate::character::Class,
    simulate::{Fightable, class::FightContext},
};

pub struct BardFightContext {
    data: FightContext,
    melody_length: i32,
    next_melody_round: i32,
    melody_dmg_multiplier: f64,
}

impl BardFightContext {
    pub fn new(data: FightContext) -> Self {
        Self {
            data,
            melody_length: -1,
            next_melody_round: 0,
            melody_dmg_multiplier: 1.0,
        }
    }

    fn melodies_attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        *round += 1;

        if self.melody_length == 0 {
            self.melody_dmg_multiplier = 1.0;
        }
        if self.melody_length <= 0 && self.next_melody_round <= 0 {
            self.assign_melodies(rng);
        }
        self.melody_length -= 1;
        self.next_melody_round -= 1;

        if !target.will_take_attack(rng) {
            return false;
        }

        let dmg = self.calculate_basic_hit_damage(*round, rng);
        target.take_attack(dmg, round, rng)
    }

    fn assign_melodies(&mut self, rng: &mut fastrand::Rng) {
        let (length, multi) = match rng.u32(0..4) {
            0 | 1 => (3, 1.4),
            2 => (3, 1.2),
            _ => (4, 1.6),
        };
        self.melody_length = length;
        self.melody_dmg_multiplier = multi;
        self.next_melody_round = 4;
    }
}

impl Fightable for BardFightContext {
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
    ) -> bool {
        if target.context().fighter.class == Class::Mage {
            self.attack_generic(target, round, rng)
        } else {
            self.melodies_attack(target, round, rng)
        }
    }

    fn reset_state(&mut self) {
        self.reset_health();
        self.melody_length = 0;
        self.next_melody_round = 0;
        self.melody_dmg_multiplier = 1.0;
    }
}
