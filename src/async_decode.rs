use crate::input::{Input, Key};
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::str::{self, Utf8Error};
use tokio::io::{self, AsyncRead, AsyncReadExt};

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
pub(crate) struct Decoder<R> {
    reader: R,
    unread_char: Option<char>,
    read_buf: String,
}

impl<R> Decoder<R> {
    pub(crate) fn new(reader: R) -> Self {
        Decoder {
            reader,
            unread_char: None,
            read_buf: String::new(),
        }
    }
}

impl<R> Decoder<R>
where
    R: AsyncRead + Unpin,
{
    async fn read_byte(&mut self) -> Result<Option<u8>> {
        let mut buf = [0];
        let byte = match self.reader.read(&mut buf).await.context(TerminalInput)? {
            0 => None,
            1 => Some(buf[0]),
            _ => panic!("never come"),
        };
        Ok(byte)
    }

    async fn read_char(&mut self) -> Result<Option<char>> {
        if let Some(ch) = self.unread_char.take() {
            return Ok(Some(ch));
        }
        let mut bytes = SmallVec::<[u8; 4]>::new();
        match self.read_byte().await? {
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
            match self.read_byte().await? {
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

    async fn read_raw_input(&mut self) -> Result<Option<Input>> {
        use Key::*;

        match self.read_char().await? {
            None => Ok(None),
            Some(esc @ '\x1b') => {
                self.read_buf.clear();
                self.read_buf.push(esc);
                let ch = match self.read_char().await? {
                    Some(ch) if ch != '[' => {
                        self.set_unread_char(ch);
                        return Ok(Some(Input::ctrl(Char('['))));
                    }
                    Some(ch) => ch,
                    None => return Ok(Some(Input::ctrl(Char('[')))),
                };

                self.read_buf.push(ch);
                while let Some(ch) = self.read_char().await? {
                    self.read_buf.push(ch);
                    match ch {
                        'A' | 'B' | 'C' | 'D' | 'H' | 'F' | '~' => break,
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

    pub(crate) async fn read_input(&mut self) -> Result<Option<Input>> {
        if let Some(input) = self.read_raw_input().await? {
            if input != Input::ctrl(Key::Char('[')) {
                return Ok(Some(input));
            }
            if let Some(mut input) = self.read_raw_input().await? {
                input.alt = true;
                return Ok(Some(input));
            }
            return Ok(Some(input));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputStrExt;
    use std::{io::Cursor, iter};

    async fn check_char(input: &str, expected: impl IntoIterator<Item = char>) {
        let mut decoder = Decoder::new(Cursor::new(input.as_bytes()));
        let mut output = vec![];
        while let Some(input) = decoder.read_char().await.unwrap() {
            output.push(input);
        }
        itertools::assert_equal(output, expected);
    }

    async fn check_input(input: &str, expected: impl IntoIterator<Item = Input>) {
        let mut decoder = Decoder::new(Cursor::new(input.as_bytes()));
        let mut output = vec![];
        while let Some(input) = decoder.read_input().await.unwrap() {
            output.push(input);
        }
        itertools::assert_equal(output, expected);
    }

    #[tokio::test]
    async fn decode_char() {
        check_char("abcdeあいうえお📝🦀", "abcdeあいうえお📝🦀".chars()).await;
    }

    #[tokio::test]
    async fn decode_input_normal() {
        use Key::*;
        check_input(
            "abcdeABCDEあいうえお📝🦀",
            "abcdeABCDEあいうえお📝🦀".chars().map(Char).map(Input::new),
        )
        .await;
    }

    #[tokio::test]
    async fn decode_input_c0_ctrl() {
        use Key::*;

        let input = (0x00..=0x1f).chain(iter::once(0x7f));
        let expected = "@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_?";
        for (input, expected) in input.zip(expected.chars()) {
            let input = String::from_utf8(vec![input]).unwrap();
            let expected = Input::ctrl(Char(expected));
            check_input(&input, iter::once(expected)).await;
        }
    }

    #[tokio::test]
    async fn decode_esc_or_alt() {
        check_input("\x1b", "C-[".inputs().map(|i| i.unwrap())).await;
        check_input("\x1ba", "M-a".inputs().map(|i| i.unwrap())).await;
        check_input("\x1b\x1b", "C-M-[".inputs().map(|i| i.unwrap())).await;
        check_input("\x1b\x00", "C-M-@".inputs().map(|i| i.unwrap())).await;
        check_input("\x1b\x05", "C-M-E".inputs().map(|i| i.unwrap())).await;
    }
}
