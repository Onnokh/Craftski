use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Copper(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coin {
    Gold,
    Silver,
    Copper,
}

/// An amount split into the denominations the game shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoinParts {
    pub gold: u64,
    pub silver: u8,
    pub copper: u8,
}

impl Copper {
    pub fn parts(self) -> CoinParts {
        CoinParts { gold: self.0 / 10_000, silver: (self.0 / 100 % 100) as u8, copper: (self.0 % 100) as u8 }
    }
}

impl CoinParts {
    /// The denominations to show, largest first: leading zero units are dropped and the units
    /// after the first are padded to two digits, as in `1g 00s 76c` and `63s 04c`.
    pub fn segments(self) -> Vec<(String, Coin)> {
        let mut out = Vec::with_capacity(3);
        if self.gold > 0 {
            out.push((self.gold.to_string(), Coin::Gold));
        }
        if !out.is_empty() {
            out.push((format!("{:02}", self.silver), Coin::Silver));
        } else if self.silver > 0 {
            out.push((self.silver.to_string(), Coin::Silver));
        }
        let copper = if out.is_empty() { self.copper.to_string() } else { format!("{:02}", self.copper) };
        out.push((copper, Coin::Copper));
        out
    }
}

impl fmt::Display for Copper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (digits, coin)) in self.parts().segments().into_iter().enumerate() {
            let unit = match coin {
                Coin::Gold => 'g',
                Coin::Silver => 's',
                Coin::Copper => 'c',
            };
            write!(f, "{}{digits}{unit}", if i > 0 { " " } else { "" })?;
        }
        Ok(())
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

    #[test]
    fn splits_into_coin_segments_for_the_coin_renderer() {
        let seg = |c: u64| Copper(c).parts().segments();
        assert_eq!(Copper(1_234_567).parts(), CoinParts { gold: 123, silver: 45, copper: 67 });
        assert_eq!(seg(10_076), vec![("1".into(), Coin::Gold), ("00".into(), Coin::Silver), ("76".into(), Coin::Copper)]);
        assert_eq!(seg(6_343), vec![("63".into(), Coin::Silver), ("43".into(), Coin::Copper)]);
        assert_eq!(seg(604), vec![("6".into(), Coin::Silver), ("04".into(), Coin::Copper)]);
        assert_eq!(seg(7), vec![("7".into(), Coin::Copper)]);
    }
}
