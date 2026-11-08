use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Copper(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coin {
    Gold,
    Silver,
    Copper,
}

impl Coin {
    pub const ALL: [Coin; 3] = [Coin::Gold, Coin::Silver, Coin::Copper];
}

/// An amount's digits in one lane per coin, gold first. An empty lane draws nothing but keeps
/// its width, so the digits of a column line up.
pub type Lanes = [Option<String>; 3];

impl Copper {
    /// Every unit from the largest non-zero one down, the units after the first padded to two
    /// digits: `1g 00s 76c`, `63s 04c`, `7c`.
    pub fn lanes(self) -> Lanes {
        let (gold, silver, copper) = (self.0 / 10_000, self.0 / 100 % 100, self.0 % 100);
        if gold > 0 {
            [Some(gold.to_string()), Some(format!("{silver:02}")), Some(format!("{copper:02}"))]
        } else if silver > 0 {
            [None, Some(silver.to_string()), Some(format!("{copper:02}"))]
        } else {
            [None, None, Some(copper.to_string())]
        }
    }

    /// The two largest units, as the table shows them: gold and silver rounded from the copper,
    /// else silver and copper, else copper alone.
    pub fn short_lanes(self) -> Lanes {
        if self.0 < 10_000 {
            return self.lanes();
        }
        let silver = (self.0 + 50) / 100;
        [Some((silver / 100).to_string()), Some(format!("{:02}", silver % 100)), None]
    }
}

impl fmt::Display for Copper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = self.lanes().into_iter().zip(['g', 's', 'c']).filter_map(|(digits, unit)| Some(format!("{}{unit}", digits?)));
        f.write_str(&units.collect::<Vec<_>>().join(" "))
    }
}

/// Signed copper amounts (profit) in the same units as [`Copper`].
pub fn signed(amount: i64) -> String {
    let sign = if amount < 0 { "-" } else { "" };
    format!("{sign}{}", Copper(amount.unsigned_abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_leading_zero_units_and_pads_the_rest() {
        assert_eq!(Copper(10_203).to_string(), "1g 02s 03c");
        assert_eq!(Copper(597).to_string(), "5s 97c");
        assert_eq!(Copper(6).to_string(), "6c");
        assert_eq!(Copper(0).to_string(), "0c");
        assert_eq!(Copper(1_230_000).to_string(), "123g 00s 00c");
    }

    #[test]
    fn signed_amounts_keep_the_minus() {
        assert_eq!(signed(-10_203), "-1g 02s 03c");
        assert_eq!(signed(45), "45c");
    }

    fn lanes(gold: Option<&str>, silver: Option<&str>, copper: Option<&str>) -> Lanes {
        [gold.map(String::from), silver.map(String::from), copper.map(String::from)]
    }

    #[test]
    fn full_lanes_keep_every_unit_below_the_largest() {
        assert_eq!(Copper(10_076).lanes(), lanes(Some("1"), Some("00"), Some("76")));
        assert_eq!(Copper(6_343).lanes(), lanes(None, Some("63"), Some("43")));
        assert_eq!(Copper(604).lanes(), lanes(None, Some("6"), Some("04")));
        assert_eq!(Copper(7).lanes(), lanes(None, None, Some("7")));
    }

    #[test]
    fn short_lanes_round_gold_amounts_to_the_silver() {
        assert_eq!(Copper(38_173).short_lanes(), lanes(Some("3"), Some("82"), None));
        assert_eq!(Copper(19_960).short_lanes(), lanes(Some("2"), Some("00"), None));
        assert_eq!(Copper(10_049).short_lanes(), lanes(Some("1"), Some("00"), None));
        assert_eq!(Copper(9_999).short_lanes(), lanes(None, Some("99"), Some("99")));
        assert_eq!(Copper(6_343).short_lanes(), lanes(None, Some("63"), Some("43")));
        assert_eq!(Copper(45).short_lanes(), lanes(None, None, Some("45")));
    }
}
