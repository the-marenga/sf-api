use crate::simulate::{Fightable, class::FightContext};

pub struct MageFightContext {
    data: FightContext,
}

impl MageFightContext {
    pub fn new(data: FightContext) -> Self {
        Self { data }
    }
}

impl Fightable for MageFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
}

pub struct WarriorFightContext {
    data: FightContext,
    block_chance: i32,
}

impl WarriorFightContext {
    pub fn new(data: FightContext, block_chance: i32) -> Self {
        Self { data, block_chance }
    }
}

impl Fightable for WarriorFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        rng.i32(1..101) > self.block_chance
    }
}

pub struct ScoutFightContext {
    data: FightContext,
}

impl ScoutFightContext {
    pub fn new(data: FightContext) -> Self {
        Self { data }
    }
}

impl Fightable for ScoutFightContext {
    fn context(&self) -> &FightContext {
        &self.data
    }

    fn context_mut(&mut self) -> &mut FightContext {
        &mut self.data
    }
    fn will_take_attack(&mut self, rng: &mut fastrand::Rng) -> bool {
        rng.i32(1..101) > 50
    }
}
