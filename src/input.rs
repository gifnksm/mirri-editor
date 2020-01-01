use matches::matches;
use snafu::Snafu;
use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult, Write as _},
    str::{self, FromStr},
};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) enum Key {
    Char(char),
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
}

impl Debug for Key {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, r#""{}""#, self)
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use Key::*;
        match self {
            Char(ch) => f.write_char(*ch),
            ArrowLeft => f.write_str("left"),
            ArrowRight => f.write_str("right"),
            ArrowUp => f.write_str("up"),
            ArrowDown => f.write_str("down"),
            Delete => f.write_str("delete"),
            Home => f.write_str("home"),
            End => f.write_str("end"),
            PageUp => f.write_str("page up"),
            PageDown => f.write_str("page down"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct ParseKeyError;
impl Display for ParseKeyError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "invalid key found in string")
    }
}
impl std::error::Error for ParseKeyError {}

impl FromStr for Key {
    type Err = ParseKeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = match s {
            "left" => Self::ArrowLeft,
            "right" => Self::ArrowRight,
            "up" => Self::ArrowUp,
            "down" => Self::ArrowDown,
            "delete" => Self::Delete,
            "home" => Self::Home,
            "end" => Self::End,
            "page up" => Self::PageUp,
            "page down" => Self::PageDown,
            _ => {
                let mut cs = s.chars();
                match (cs.next(), cs.next()) {
                    (Some(ch), None) => Self::Char(ch),
                    _ => return Err(ParseKeyError),
                }
            }
        };
        Ok(key)
    }
}

impl Key {
    fn need_angle_bracket(&self) -> bool {
        !matches!(self, Key::Char(_))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct Input {
    pub(crate) key: Key,
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
}

impl Debug for Input {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, r#""{}""#, self)
    }
}

impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let need_angle_bracket = self.key.need_angle_bracket();
        if need_angle_bracket {
            write!(f, "<")?;
        }
        if self.ctrl {
            write!(f, "C-")?;
        }
        if self.alt {
            write!(f, "M-")?;
        }
        write!(f, "{}", self.key)?;
        if need_angle_bracket {
            write!(f, ">")?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Snafu)]
pub(crate) enum ParseInputError {
    #[snafu(display("invalid key found in string"))]
    InvalidKey,
    #[snafu(display("unneeded angle bracket found in string"))]
    UnneededAngleBracket,
    #[snafu(display("no angle bracket found in string"))]
    NoAngleBracket,
}

impl FromStr for Input {
    type Err = ParseInputError;
    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        let has_bracket = s.starts_with('<') && s.ends_with('>');
        if has_bracket {
            s = &s[1..s.len() - 1];
        }
        let mut ctrl = false;
        let mut alt = false;
        loop {
            if s.starts_with("C-") {
                ctrl = true;
                s = &s[2..];
                continue;
            }
            if s.starts_with("M-") {
                alt = true;
                s = &s[2..];
                continue;
            }
            break;
        }
        let key = Key::from_str(s).map_err(|_| ParseInputError::InvalidKey)?;
        if has_bracket != key.need_angle_bracket() {
            if has_bracket {
                return Err(ParseInputError::UnneededAngleBracket);
            } else {
                return Err(ParseInputError::NoAngleBracket);
            }
        }
        Ok(Input { ctrl, alt, key })
    }
}

impl Input {
    pub(crate) fn new(key: Key) -> Self {
        Input {
            key,
            ctrl: false,
            alt: false,
        }
    }
    pub(crate) fn ctrl(key: Key) -> Self {
        Input {
            key,
            ctrl: true,
            alt: false,
        }
    }
}

pub(crate) trait InputStrExt {
    type Iter;
    fn inputs(&self) -> Self::Iter;
}

impl<'a> InputStrExt for &'a str {
    type Iter = Inputs<'a>;
    fn inputs(&self) -> Self::Iter {
        Inputs {
            s: self.trim_start(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Inputs<'a> {
    s: &'a str,
}

impl<'a> Iterator for Inputs<'a> {
    type Item = Result<Input, ParseInputError>;
    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(!self.s.starts_with(char::is_whitespace));
        if self.s.is_empty() {
            return None;
        }

        let len = if self.s.starts_with('<') {
            self.s.find('>').map(|idx| idx + 1)
        } else {
            self.s.find(char::is_whitespace)
        }
        .unwrap_or_else(|| self.s.len());

        let input = self.s[..len].parse();
        self.s = &self.s[len..].trim_start();
        Some(input)
    }
}

impl<'a> DoubleEndedIterator for Inputs<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        debug_assert!(!self.s.ends_with(char::is_whitespace));
        if self.s.is_empty() {
            return None;
        }

        let start = if self.s.ends_with('>') {
            self.s.rfind('<')
        } else {
            self.s.rfind(char::is_whitespace)
        }
        .unwrap_or(0);

        let input = self.s[start..].parse();
        self.s = &self.s[..start].trim_end();
        Some(input)
    }
}

impl<'a> std::iter::FusedIterator for Inputs<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_key() {
        fn check(key: Key) {
            let k2 = Key::from_str(&key.to_string()).unwrap();
            assert_eq!(key, k2);
        }
        check(Key::Char('a'));
        check(Key::ArrowLeft);
        check(Key::ArrowRight);
        check(Key::ArrowUp);
        check(Key::ArrowDown);
        check(Key::Delete);
        check(Key::Home);
        check(Key::End);
        check(Key::PageUp);
        check(Key::PageDown);
    }

    #[test]
    fn parse_key() {
        assert!(Key::from_str("aaa").is_err());
    }

    #[test]
    fn convert_input() {
        fn check(s: &str) {
            let s2 = Input::from_str(s).unwrap().to_string();
            assert_eq!(s, s2);
        }
        check("a");
        check("C-a");
        check("M-a");
        check("C-M-a");
        check("<page up>");
        check("<C-page up>");
        check("<M-page up>");
        check("<C-M-page up>");
    }

    #[test]
    fn parse_input() {
        fn check(s: &str, e: ParseInputError) {
            assert_eq!(Input::from_str(s), Err(e));
        }
        check("aaa", ParseInputError::InvalidKey);
        check("C-M-page up", ParseInputError::NoAngleBracket);
        check("<C-M-a>", ParseInputError::UnneededAngleBracket);
    }

    #[test]
    fn str_inputs() {
        assert!("a b c"
            .inputs()
            .eq(vec!["a".parse(), "b".parse(), "c".parse()]));
        assert!("<a b>  b c <page up>".inputs().eq(vec![
            "<a b>".parse(),
            "b".parse(),
            "c".parse(),
            "<page up>".parse()
        ]));
        assert!("    ".inputs().eq(vec![]));
    }
}
