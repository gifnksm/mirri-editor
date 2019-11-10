use crate::row::Row;
use std::iter;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Highlight {
    Normal,
    Number,
    Match,
}

impl Highlight {
    pub(crate) fn to_color(self) -> u32 {
        match self {
            Self::Normal => 37,
            Self::Number => 31,
            Self::Match => 34,
        }
    }
}

impl Row {
    pub(crate) fn update_syntax(&mut self) {
        self.hl = self
            .render
            .chars()
            .flat_map(|ch| {
                let byte = ch.len_utf8();
                let hl = if ch.is_digit(10) {
                    Highlight::Number
                } else {
                    Highlight::Normal
                };
                iter::repeat(hl).take(byte)
            })
            .collect();
    }
}
