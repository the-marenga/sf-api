use crate::{error::Result, response::Vals};

/// The unlocked toilet, that you can throw items into
#[derive(Debug, Clone, Default, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Toilet {
    /// The level the aura is at currently
    pub aura: u32,
    /// The amount of mana currently in the toilet
    pub mana_currently: u32,
    /// The total amount of mana you have to collect to flush the toilet
    pub mana_total: u32,
    /// The amount of sacrifices you have left today
    pub sacrifices_left: u32,
}

impl Toilet {
    /// Updates the toilet state from a 4-value response: `[aura,
    /// mana_current, timestamp, mana_total]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        self.aura = data.u32(0)?;
        self.mana_currently = data.u32(1)?;
        data.ignore(2, "unknown server timestamp (last flush / item time)");
        self.mana_total = data.u32(3)?;
        Ok(())
    }

    /// Updates the sacrifices left from a 3-value `toiletstate` response.
    pub(crate) fn update_sacrifices(&mut self, data: &Vals<'_>) {
        data.ignore(0..2, "unknown toilet state fields");
        self.sacrifices_left = data.u32_or(2, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toilet_update() {
        let raw = "12/500/1600000000/1000";
        let data = Vals::from(raw);
        let mut toilet = Toilet::default();
        toilet.update(&data).unwrap();
        assert_eq!(toilet.aura, 12);
        assert_eq!(toilet.mana_currently, 500);
        assert_eq!(toilet.mana_total, 1000);
        data.assert_all_read();
    }

    #[test]
    fn test_toilet_update_sacrifices() {
        let raw = "0/0/3";
        let data = Vals::from(raw);
        let mut toilet = Toilet::default();
        toilet.update_sacrifices(&data);
        assert_eq!(toilet.sacrifices_left, 3);
        data.assert_all_read();
    }
}
