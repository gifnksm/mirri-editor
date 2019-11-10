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
        self.hl.clear();

        let mut prev_sep = true;
        let mut prev_hl = Highlight::Normal;

        for ch in self.render.chars() {
            let (hl, is_sep) = if ch.is_digit(10) && (prev_sep || prev_hl == Highlight::Number)
                || (ch == '.' && prev_hl == Highlight::Number)
            {
                (Highlight::Number, false)
            } else {
                (Highlight::Normal, is_separator(ch))
            };

            self.hl.extend(iter::repeat(hl).take(ch.len_utf8()));

            prev_hl = hl;
            prev_sep = is_sep;
        }
    }
}

fn is_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '\0' || ",.()+-/*=~%<>[];".contains(ch)
}
