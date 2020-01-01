use crate::input::{Input, Key};
use log::{trace, warn};
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    fmt::Debug,
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

    fn read_char(&mut self, reader: &mut impl Read) -> Result<Option<char>> {
        if let Some(ch) = self.unread_char.take() {
            trace!("read_char (from unread): Some({:?})", ch);
            return Ok(Some(ch));
        }
        let mut bytes = SmallVec::<[u8; 4]>::new();
        match self.read_byte(reader)? {
            Some(b) => bytes.push(b),
            None => {
                trace!("read_char: None");
                return Ok(None);
            }
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
        let ch = s.chars().next();
        trace!("read_char: {:?}", ch);
        Ok(ch)
    }

    fn set_unread_char(&mut self, ch: char) {
        assert!(self.unread_char.is_none());
        self.unread_char = Some(ch);
    }

    fn read_raw_input(&mut self, reader: &mut impl Read) -> Result<Option<Input>> {
        use Key::*;

        match self.read_char(reader)? {
            None => Ok(None),
            Some(esc @ '\x1b') => {
                self.read_buf.clear();
                self.read_buf.push(esc);
                let ch = match self.read_char(reader)? {
                    Some(ch) if ch != '[' => {
                        self.set_unread_char(ch);
                        return Ok(Some(Input::ctrl(Char('['))));
                    }
                    Some(ch) => ch,
                    None => return Ok(Some(Input::ctrl(Char('[')))),
                };

                self.read_buf.push(ch);
                while let Some(ch) = self.read_char(reader)? {
                    self.read_buf.push(ch);
                    match ch {
                        'A' | 'B' | 'C' | 'D' | 'H' | 'F' | 'Z' | '~' => break,
                        _ => continue,
                    }
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
                    seq => {
                        warn!("read_char_raw: unknown seq {:?}", seq);
                        return Ok(Some(Input::new(Char('?'))));
                    }
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

    pub(crate) fn read_input(&mut self, reader: &mut impl Read) -> Result<Option<Input>> {
        if let Some(input) = self.read_raw_input(reader)? {
            if input != Input::ctrl(Key::Char('[')) {
                trace!("read_input: Some({:?})", input);
                return Ok(Some(input));
            }
            if let Some(mut input) = self.read_raw_input(reader)? {
                input.alt = true;
                trace!("read_input: Some({:?})", input);
                return Ok(Some(input));
            }
            trace!("read_input: Some({:?})", input);
            return Ok(Some(input));
        }
        trace!("read_input: None");
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputStrExt;
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

    #[test]
    fn decode_esc_or_alt() {
        fn check(input: &str, expected: Vec<Input>) {
            let mut decoder = Decoder::new();
            let mut output = vec![];
            let mut cur = Cursor::new(input.as_bytes());
            while let Ok(Some(input)) = decoder.read_input(&mut cur) {
                output.push(input);
            }
            assert_eq!(output, expected);
        }

        check("\x1b", "C-[".inputs().map(|i| i.unwrap()).collect());
        check("\x1ba", "M-a".inputs().map(|i| i.unwrap()).collect());
        check("\x1b\x1b", "C-M-[".inputs().map(|i| i.unwrap()).collect());
        check("\x1b\x00", "C-M-@".inputs().map(|i| i.unwrap()).collect());
        check("\x1b\x05", "C-M-E".inputs().map(|i| i.unwrap()).collect());
    }
}
