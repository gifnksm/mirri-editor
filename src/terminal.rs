use crate::signal::SignalReceiver;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    io::{self, Read, Stdin, Stdout, Write},
    mem,
    os::unix::io::AsRawFd,
    str::{self, Utf8Error},
};
use termios::Termios;

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not enter raw mode: {}", source))]
    EnterRawMode {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not initialize signal receiver: {:?}", source))]
    SignalReceiverInit {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not read from terminal: {}", source))]
    TerminalInput {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not write to terminal: {}", source))]
    TerminalOutput {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not handle non-UTF8 input sequence: {}", source))]
    NonUtf8Input {
        source: Utf8Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Unecptected escape sequence: {:?}", seq))]
    UnexpectedEscapeSequence { backtrace: Backtrace, seq: String },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Key {
    Char(char),
    Backspace,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct Input {
    pub(crate) key: Key,
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
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

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub(crate) struct RawTerminal {
    stdin: Stdin,
    stdout: Stdout,
    pub(crate) screen_cols: usize,
    pub(crate) screen_rows: usize,
    sigwinch_receiver: SignalReceiver,
    orig_termios: Termios,
    unread_char: Option<char>,
    read_buf: String,
}

impl RawTerminal {
    pub(crate) fn new() -> Result<Self> {
        use termios::*;

        let stdin = io::stdin();
        let stdout = io::stdout();

        let fd = stdin.as_raw_fd();
        let mut raw = Termios::from_fd(fd).context(EnterRawMode)?;
        let orig_termios = raw;

        // See termios(3) for detail.

        // Set input flags:
        // * `!BRKINT` : disable break condition
        // * `!ICRNL`  : disable CR to NL translation
        // * `!INPCK`  : disable input parity checking
        // * `!ISTRIP` : disable stripping off eighth bit
        // * `!IXON`   : disable software flow control (Ctrl-Q, Ctrl-S)
        raw.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);

        // Set output flags:
        // * `!OPOST` : disable output processing such as "\n" to "\r\n" translation.
        raw.c_oflag &= !OPOST;

        // Set control flags:
        // * `CS8` : set character size as 8
        raw.c_cflag |= CS8;

        // Set local flags:
        // * `!ECHO`   : disable echoing
        // * `!ICANON` : disable canonical mode
        // * `!IEXTEN` : disable input processing such as Ctrl-V
        // * `!ISIG`   : disable generating the signal when receiving INTR (Ctrl-C), QUIT (Ctrl-\), SUSP (Ctrl-Z), or DSUSP (Ctrl-Y).
        raw.c_lflag &= !(ECHO | ICANON | IEXTEN | ISIG);

        // Set control characters
        raw.c_cc[VMIN] = 0; // minimum number of bytes of input needed before `read()`
        raw.c_cc[VTIME] = 1; // maximum amount of time to wait before `read()` returns

        tcsetattr(fd, TCSAFLUSH, &raw).context(EnterRawMode)?;

        let sigwinch_receiver = SignalReceiver::new_sigwinch().context(SignalReceiverInit)?;

        let mut term = Self {
            stdin,
            stdout,
            screen_cols: 0,
            screen_rows: 0,
            sigwinch_receiver,
            orig_termios,
            unread_char: None,
            read_buf: String::new(),
        };
        term.update_screen_size()?;
        Ok(term)
    }

    fn read_byte(&mut self) -> Result<Option<u8>> {
        let mut buf = [0];
        let byte = match self.stdin.read(&mut buf).context(TerminalInput)? {
            0 => None,
            1 => Some(buf[0]),
            _ => panic!("never come"),
        };
        Ok(byte)
    }

    fn read_char(&mut self) -> Result<Option<char>> {
        if let Some(ch) = self.unread_char.take() {
            return Ok(Some(ch));
        }
        let mut bytes = SmallVec::<[u8; 4]>::new();
        match self.read_byte()? {
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
            match self.read_byte()? {
                Some(b) => bytes.push(b),
                None => break,
            }
        }

        let s = str::from_utf8(&bytes).context(NonUtf8Input)?;
        let mut cs = s.chars();
        let ch = cs.next();
        debug_assert!(ch.is_none() || cs.next().is_none());
        Ok(dbg!(ch))
    }

    fn set_unread_char(&mut self, ch: char) {
        assert!(self.unread_char.is_none());
        self.unread_char = Some(ch);
    }

    pub(crate) fn read_input(&mut self) -> Result<Option<Input>> {
        use Key::*;

        match self.read_char()? {
            None => Ok(None),
            Some(esc @ '\x1b') => {
                self.read_buf.clear();
                self.read_buf.push(esc);
                let ch = match self.read_char()? {
                    Some(ch) => ch,
                    None => return Ok(Some(Input::ctrl(Char('[')))),
                };
                if ch == '[' {
                    self.read_buf.push(ch);
                    while let Some(ch) = self.read_char()? {
                        self.read_buf.push(ch);
                        match ch {
                            'A' | 'B' | 'C' | 'D' | 'H' | 'F' | '~' => break,
                            _ => continue,
                        }
                    }
                } else {
                    self.set_unread_char(ch);
                    let mut input = self.read_input()?;
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
            Some('\x7f') => Ok(Some(Input::new(Backspace))),
            Some(ch) if ch.is_ascii_control() => {
                assert!((ch as u8) < 0x1f);
                let key = Key::Char((ch as u8 + 0x40) as char);
                Ok(Some(Input::ctrl(key)))
            }
            Some(ch) => Ok(Some(Input::new(Char(ch)))),
        }
    }

    pub(crate) fn maybe_update_screen_size(&mut self) -> Result<()> {
        if self.sigwinch_receiver.received() {
            self.update_screen_size()?;
        }
        Ok(())
    }

    fn update_screen_size(&mut self) -> Result<()> {
        let (screen_cols, screen_rows) = self.get_window_size()?;
        self.screen_cols = screen_cols;
        self.screen_rows = screen_rows - 2; // status bar height + message bar height
        Ok(())
    }

    fn get_window_size(&mut self) -> Result<(usize, usize)> {
        if let Some(sz) = term_size::dimensions() {
            return Ok(sz);
        }

        // Move the cursor to the bottom-right corner.
        // `<esc>[9999;9999H` cannot be used here because the it does not
        // guarantee that the cursor stops on the corner.
        write!(&mut self.stdout, "\x1b[9999C\x1b[9999B").context(TerminalOutput)?;
        // Query the cursor position
        write!(&mut self.stdout, "\x1b[6n").context(TerminalOutput)?;

        self.stdout.flush().context(TerminalOutput)?;

        // Read the cursor position
        self.read_buf.clear();
        while let Some(ch) = self.read_char()? {
            self.read_buf.push(ch);
            if ch == 'R' {
                break;
            }
        }

        let s = self.read_buf.trim_end_matches('R');
        if s.starts_with("\x1b[") {
            let s = s.trim_start_matches("\x1b[");
            let mut it = s.split(';');
            let row = it.next().and_then(|s| s.parse().ok());
            let col = it.next().and_then(|s| s.parse().ok());
            let next = it.next();
            if let (Some(row), Some(col), None) = (row, col, next) {
                return Ok((col, row));
            }
        }

        UnexpectedEscapeSequence {
            seq: mem::replace(&mut self.read_buf, String::new()),
        }
        .fail()
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        use termios::*;
        let fd = self.stdin.as_raw_fd();
        tcsetattr(fd, TCSAFLUSH, &self.orig_termios).expect("failed to restore terminal mode");
    }
}

impl Write for RawTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
