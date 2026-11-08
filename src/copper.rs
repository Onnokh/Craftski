use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Copper(pub u64);

impl fmt::Display for Copper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (g, s, c) = (self.0 / 10_000, self.0 / 100 % 100, self.0 % 100);
        if g > 0 {
            write!(f, "{g}g {s:02}s {c:02}c")
        } else if s > 0 {
            write!(f, "{s}s {c:02}c")
        } else {
            write!(f, "{c}c")
        }
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
}
