//! Cards, suits and colours.
//!
//! A [`Card`] is one byte: the low six bits are its face (suit × 13 + rank − 1) and the top bit
//! says whether it lies face down. Cards from different packs are deliberately indistinguishable:
//! for solving, two 7♥s are the same card, which lets the solver merge positions that differ
//! only in which copy sits where. Jokers are one more face, [`JOKER`]; all jokers are alike.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A suit of the French pack.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Hearts = 2,
    Spades = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn from_index(i: u8) -> Suit {
        Suit::ALL[(i & 3) as usize]
    }

    pub fn color(self) -> Color {
        match self {
            Suit::Diamonds | Suit::Hearts => Color::Red,
            Suit::Clubs | Suit::Spades => Color::Black,
        }
    }

    pub fn letter(self) -> char {
        match self {
            Suit::Clubs => 'C',
            Suit::Diamonds => 'D',
            Suit::Hearts => 'H',
            Suit::Spades => 'S',
        }
    }

    pub fn from_letter(c: char) -> Option<Suit> {
        match c.to_ascii_uppercase() {
            'C' => Some(Suit::Clubs),
            'D' => Some(Suit::Diamonds),
            'H' => Some(Suit::Hearts),
            'S' => Some(Suit::Spades),
            _ => None,
        }
    }
}

/// Red or black.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Red,
    Black,
}

const FACE_DOWN: u8 = 0x80;
const FACE_MASK: u8 = 0x3f;

/// Number of distinct faces: the 52 of a French pack, then the joker.
pub const FACES: usize = 53;

/// The joker's face index. A joker has no suit or rank.
pub const JOKER: u8 = 52;

/// Faces 0..52 as a bit set: every card but the joker.
pub(crate) const REAL_FACES: u64 = (1 << 52) - 1;

/// A playing card, face up or face down.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Card(u8);

impl Card {
    /// `rank` is 1 (ace) to 13 (king).
    pub fn new(suit: Suit, rank: u8) -> Card {
        debug_assert!((1..=13).contains(&rank));
        Card(suit as u8 * 13 + rank - 1)
    }

    /// A joker, face up.
    pub const fn joker() -> Card {
        Card(JOKER)
    }

    /// The card for a face index (0..53, 52 being the joker), face up.
    pub fn from_face(face: u8) -> Card {
        debug_assert!((face as usize) < FACES);
        Card(face)
    }

    #[inline]
    pub fn is_joker(self) -> bool {
        self.face() == JOKER
    }

    /// The face index, 0..53, independent of orientation.
    #[inline]
    pub fn face(self) -> u8 {
        self.0 & FACE_MASK
    }

    /// Meaningless for a joker (which has none): check [`Card::is_joker`] first.
    #[inline]
    pub fn suit(self) -> Suit {
        Suit::from_index(self.face() / 13)
    }

    /// 1 (ace) to 13 (king); 0 for a joker.
    #[inline]
    pub fn rank(self) -> u8 {
        let f = self.face();
        if f == JOKER {
            0
        } else {
            f % 13 + 1
        }
    }

    #[inline]
    pub fn color(self) -> Color {
        self.suit().color()
    }

    #[inline]
    pub fn is_face_up(self) -> bool {
        self.0 & FACE_DOWN == 0
    }

    #[inline]
    pub fn up(self) -> Card {
        Card(self.0 & !FACE_DOWN)
    }

    #[inline]
    pub fn down(self) -> Card {
        Card(self.0 | FACE_DOWN)
    }

    #[inline]
    pub fn flipped(self) -> Card {
        Card(self.0 ^ FACE_DOWN)
    }

    /// The raw byte: face in the low six bits, face-down flag in the top bit.
    #[inline]
    pub fn to_byte(self) -> u8 {
        self.0
    }
}

const RANK_CHARS: &[u8; 13] = b"A23456789TJQK";

impl fmt::Display for Card {
    /// `AS`, `TD`, `KH`, `JK` (a joker); a face-down card is prefixed with `~` (`~QC`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_face_up() {
            f.write_str("~")?;
        }
        if self.is_joker() {
            return f.write_str("JK");
        }
        write!(
            f,
            "{}{}",
            RANK_CHARS[self.rank() as usize - 1] as char,
            self.suit().letter()
        )
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Error parsing a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCardError(pub String);

impl fmt::Display for ParseCardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a card: {:?} (expected e.g. \"AS\", \"10H\", \"TD\", \"~KC\", \"JK\")",
            self.0
        )
    }
}

impl std::error::Error for ParseCardError {}

impl FromStr for Card {
    type Err = ParseCardError;

    /// Accepts `AS`, `as`, `10S`, `TS`, `JK` (a joker), and a leading `~` for face down.
    fn from_str(s: &str) -> Result<Card, ParseCardError> {
        let err = || ParseCardError(s.to_string());
        let t = s.trim();
        let (down, t) = match t.strip_prefix('~') {
            Some(rest) => (true, rest),
            None => (false, t),
        };
        if t.eq_ignore_ascii_case("JK") || t.eq_ignore_ascii_case("joker") {
            let j = Card::joker();
            return Ok(if down { j.down() } else { j });
        }
        let mut chars: Vec<char> = t.chars().collect();
        let suit = Suit::from_letter(chars.pop().ok_or_else(err)?).ok_or_else(err)?;
        let rank_str: String = chars.into_iter().collect::<String>().to_ascii_uppercase();
        let rank = match rank_str.as_str() {
            "A" | "1" => 1,
            "T" | "10" => 10,
            "J" | "11" => 11,
            "Q" | "12" => 12,
            "K" | "13" => 13,
            r => match r.parse::<u8>() {
                Ok(n) if (2..=9).contains(&n) => n,
                _ => return Err(err()),
            },
        };
        let c = Card::new(suit, rank);
        Ok(if down { c.down() } else { c })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for face in 0..52u8 {
            let c = Card::from_face(face);
            assert_eq!(c.to_string().parse::<Card>().unwrap(), c);
            assert_eq!(c.down().to_string().parse::<Card>().unwrap(), c.down());
            assert_eq!(c.down().face(), face);
        }
        assert_eq!("10h".parse::<Card>().unwrap(), Card::new(Suit::Hearts, 10));
        assert!("1X".parse::<Card>().is_err());
        let j: Card = "JK".parse().unwrap();
        assert!(j.is_joker() && j.rank() == 0);
        assert_eq!(j.to_string(), "JK");
        assert_eq!("~jk".parse::<Card>().unwrap(), j.down());
    }
}
