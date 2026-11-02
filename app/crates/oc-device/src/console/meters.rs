//! `LVI` / `LVO`: peak hold since the previous read, integer dBFS, `-99` = silence.

use super::Dsp;
use crate::error::{Error, Result};

/// Inputs: USB L, R, C, LFE, Ls, Rs, AUX L, R.
pub const INPUTS: usize = 8;
/// Outputs: inner L, inner R, outer L, outer R, bass A, bass B.
pub const OUTPUTS: usize = 6;
pub const SILENCE_DB: i8 = -99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meters {
    pub peak_in: [i8; INPUTS],
    pub peak_out: [i8; OUTPUTS],
}

impl Default for Meters {
    fn default() -> Self {
        Self {
            peak_in: [SILENCE_DB; INPUTS],
            peak_out: [SILENCE_DB; OUTPUTS],
        }
    }
}

pub fn parse_levels<const N: usize>(s: &str) -> Option<[i8; N]> {
    let mut out = [SILENCE_DB; N];
    let mut n = 0;
    for (i, v) in s.trim().split(',').enumerate() {
        *out.get_mut(i)? = v.trim().parse::<i32>().ok()?.clamp(-128, 127) as i8;
        n += 1;
    }
    (n == N).then_some(out)
}

impl Dsp<'_> {
    fn levels<const N: usize>(&self, cmd: &'static str) -> Result<[i8; N]> {
        let s = self.text(cmd)?;
        parse_levels(&s).ok_or(Error::Reply { cmd, reply: s })
    }

    pub fn peak_in(&self) -> Result<[i8; INPUTS]> {
        self.levels("LVI")
    }

    pub fn peak_out(&self) -> Result<[i8; OUTPUTS]> {
        self.levels("LVO")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_parse() {
        assert_eq!(parse_levels::<6>("-73,-99,-73,-99,-99,-99"), Some([-73, -99, -73, -99, -99, -99]));
        assert_eq!(parse_levels::<3>("1,2"), None);
        assert_eq!(parse_levels::<2>("1,2,3"), None);
        assert_eq!(parse_levels::<2>("x,2"), None);
    }
}
