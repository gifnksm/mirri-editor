use matches::matches;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult, Write as _},
    io::{self, Read},
    str::{self, FromStr, Utf8Error},
};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not read from terminal: {}", source))]
    TerminalInput {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not handle non-UTF8 input sequence: {}", source))]
    NonUtf8Input {
        source: Utf8Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Unexpected escape sequence: {:?}", seq))]
    UnexpectedEscapeSequence { backtrace: Backtrace, seq: String },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

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
    fn new(key: Key) -> Self {
        Input {
            key,
            ctrl: false,
            alt: false,
        }
    }
    fn ctrl(key: Key) -> Self {
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

#[derive(Debug)]
pub(crate) struct Decoder {
    unread_char: Option<char>,
    read_buf: String,
}

impl Decoder {
    pub(crate) fn new() -> Self {
        Decoder {
            unread_char: None,
            read_buf: String::new(),
        }
    }

    fn read_byte(&mut self, reader: &mut impl Read) -> Result<Option<u8>> {
        let mut buf = [0];
        let byte = match reader.read(&mut buf).context(TerminalInput)? {
            0 => None,
            1 => Some(buf[0]),
            _ => panic!("never come"),
        };
        Ok(byte)
    }

    pub(crate) fn read_char(&mut self, reader: &mut impl Read) -> Result<Option<char>> {
        if let Some(ch) = self.unread_char.take() {
            return Ok(Some(ch));
        }
        let mut bytes = SmallVec::<[u8; 4]>::new();
        match self.read_byte(reader)? {
            Some(b) => bytes.push(b),
            None => return Ok(None),
        };

        // https://tools.ietf.org/html/rfc3629
        let width = match bytes[0] {
            0b0000_0000..=0b0111_1111 => 1,
            0b1000_0000..=0b1011_1111 | 0b1111_1000..=0b1111_1111 => 0,
            0b1100_0000..=0b1101_1111 => 2,
            0b1110_0000..=0b1110_1111 => 3,
            0b1111_0000..=0b1111_0111 => 4,
        };

        while bytes.len() < width {
            match self.read_byte(reader)? {
                Some(b) => bytes.push(b),
                None => break,
            }
        }

        let s = str::from_utf8(&bytes).context(NonUtf8Input)?;
        Ok(s.chars().next())
    }

    fn set_unread_char(&mut self, ch: char) {
        assert!(self.unread_char.is_none());
        self.unread_char = Some(ch);
    }

    pub(crate) fn read_input(&mut self, reader: &mut impl Read) -> Result<Option<Input>> {
        use Key::*;

        match self.read_char(reader)? {
            None => Ok(None),
            Some(esc @ '\x1b') => {
                self.read_buf.clear();
                self.read_buf.push(esc);
                let ch = match self.read_char(reader)? {
                    Some(ch) => ch,
                    None => return Ok(Some(Input::ctrl(Char('[')))),
                };
                if ch == '[' {
                    self.read_buf.push(ch);
                    while let Some(ch) = self.read_char(reader)? {
                        self.read_buf.push(ch);
                        match ch {
                            'A' | 'B' | 'C' | 'D' | 'H' | 'F' | '~' => break,
                            _ => continue,
                        }
                    }
                } else {
                    self.set_unread_char(ch);
                    let mut input = self.read_input(reader)?;
                    if let Some(input) = &mut input {
                        input.alt = true;
                    }
                    return Ok(input);
                }
                let key = match &self.read_buf[..] {
                    "\x1b[1~" | "\x1b[7~" | "\x1b[H" => Home,
                    "\x1b[3~" => Delete,
                    "\x1b[4~" | "\x1b[8~" | "\x1b[F" => End,
                    "\x1b[5~" => PageUp,
                    "\x1b[6~" => PageDown,
                    "\x1b[A" => ArrowUp,
                    "\x1b[B" => ArrowDown,
                    "\x1b[C" => ArrowRight,
                    "\x1b[D" => ArrowLeft,
                    _ => return Ok(Some(Input::ctrl(Char('[')))),
                };
                Ok(Some(Input::new(key)))
            }
            Some(ch) if ch.is_ascii_control() => {
                let key = Key::Char((ch as u8 ^ 0x40) as char);
                Ok(Some(Input::ctrl(key)))
            }
            Some(ch) => Ok(Some(Input::new(Char(ch)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, iter};

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

    #[test]
    fn decode_char() {
        let input = "abcdeあいうえお📝🦀";
        let mut decoder = Decoder::new();
        let mut output = vec![];
        let mut cur = Cursor::new(input.as_bytes());
        while let Ok(Some(ch)) = decoder.read_char(&mut cur) {
            output.push(ch);
        }
        assert_eq!(
            output,
            &['a', 'b', 'c', 'd', 'e', 'あ', 'い', 'う', 'え', 'お', '📝', '🦀']
        );
    }

    #[test]
    fn decode_input_normal() {
        use Key::*;

        let input = "abcdeABCDEあいうえお📝🦀";
        let mut decoder = Decoder::new();
        let mut output = vec![];
        let mut cur = Cursor::new(input.as_bytes());
        while let Ok(Some(input)) = decoder.read_input(&mut cur) {
            output.push(input);
        }
        assert_eq!(
            output,
            &[
                Input::new(Char('a')),
                Input::new(Char('b')),
                Input::new(Char('c')),
                Input::new(Char('d')),
                Input::new(Char('e')),
                Input::new(Char('A')),
                Input::new(Char('B')),
                Input::new(Char('C')),
                Input::new(Char('D')),
                Input::new(Char('E')),
                Input::new(Char('あ')),
                Input::new(Char('い')),
                Input::new(Char('う')),
                Input::new(Char('え')),
                Input::new(Char('お')),
                Input::new(Char('📝')),
                Input::new(Char('🦀'))
            ]
        );
    }

    #[test]
    fn decode_input_c0_ctrl() {
        use Key::*;

        let input = (0x00..=0x1f).chain(iter::once(0x7f));
        let expected = [
            '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P',
            'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', '?',
        ]
        .iter()
        .map(|ch| Input::ctrl(Char(*ch)))
        .collect::<Vec<_>>();

        let mut decoder = Decoder::new();
        let mut output = vec![];
        for input in input {
            let mut cur = Cursor::new(vec![input]);
            while let Ok(Some(input)) = decoder.read_input(&mut cur) {
                output.push(input);
            }
        }
        itertools::assert_equal(output, expected);
    }
}
