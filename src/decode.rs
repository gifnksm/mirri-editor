use matches::matches;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write as _},
    io::{self, Read},
    str::{self, Utf8Error},
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct Input {
    pub(crate) key: Key,
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
}

impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let need_escape = !matches!(self.key, Key::Char(_));
        if need_escape {
            write!(f, "<")?;
        }
        if self.ctrl {
            write!(f, "C-")?;
        }
        if self.alt {
            write!(f, "M-")?;
        }
        write!(f, "{}", self.key)?;
        if need_escape {
            write!(f, ">")?;
        }
        Ok(())
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
