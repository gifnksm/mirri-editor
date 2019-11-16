use crate::{
    decode::{self, Decoder},
    geom::Size,
    signal::SignalReceiver,
};
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    io::{self, Read, Stdin, Stdout, Write},
    mem,
    os::unix::io::AsRawFd,
    str,
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
    #[snafu(display("{}", source))]
    Decode {
        source: decode::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not write to terminal: {}", source))]
    TerminalOutput {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Unecptected escape sequence: {:?}", seq))]
    UnexpectedEscapeSequence { backtrace: Backtrace, seq: String },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub(crate) struct RawTerminal {
    stdin: Stdin,
    stdout: Stdout,
    pub(crate) screen_size: Size,
    sigwinch_receiver: SignalReceiver,
    orig_termios: Termios,
}

impl RawTerminal {
    pub(crate) fn new(decoder: &mut Decoder) -> Result<Self> {
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
            screen_size: Size::default(),
            sigwinch_receiver,
            orig_termios,
        };

        term.update_screen_size(decoder)?;

        Ok(term)
    }

    pub(crate) fn maybe_update_screen_size(&mut self, decoder: &mut Decoder) -> Result<bool> {
        let need_update = self.sigwinch_receiver.received();
        if need_update {
            self.update_screen_size(decoder)?;
        }
        Ok(need_update)
    }

    fn update_screen_size(&mut self, decoder: &mut Decoder) -> Result<()> {
        self.screen_size = self.get_window_size(decoder)?;
        Ok(())
    }

    fn get_window_size(&mut self, decoder: &mut Decoder) -> Result<Size> {
        if let Some((cols, rows)) = term_size::dimensions() {
            return Ok(Size { cols, rows });
        }

        // Move the cursor to the bottom-right corner.
        // `<esc>[9999;9999H` cannot be used here because the it does not
        // guarantee that the cursor stops on the corner.
        write!(&mut self.stdout, "\x1b[9999C\x1b[9999B").context(TerminalOutput)?;
        // Query the cursor position
        write!(&mut self.stdout, "\x1b[6n").context(TerminalOutput)?;

        self.stdout.flush().context(TerminalOutput)?;

        // Read the cursor position
        let mut buf = String::new();
        while let Some(ch) = decoder.read_char(&mut self.stdin).context(Decode)? {
            buf.push(ch);
            if ch == 'R' {
                break;
            }
        }

        let s = buf.trim_end_matches('R');
        if s.starts_with("\x1b[") {
            let s = s.trim_start_matches("\x1b[");
            let mut it = s.split(';');
            let rows = it.next().and_then(|s| s.parse().ok());
            let cols = it.next().and_then(|s| s.parse().ok());
            let next = it.next();
            if let (Some(rows), Some(cols), None) = (rows, cols, next) {
                return Ok(Size { cols, rows });
            }
        }

        UnexpectedEscapeSequence {
            seq: mem::replace(&mut buf, String::new()),
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

impl Read for RawTerminal {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdin.read(buf)
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
