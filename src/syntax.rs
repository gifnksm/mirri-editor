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
