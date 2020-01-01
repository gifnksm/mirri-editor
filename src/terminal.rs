use crate::{geom::Size, signal::SignalReceiver};
use nix::sys::termios::{self, SetArg, Termios};
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    io::{self, Read, Stdin, Stdout, Write},
    os::unix::io::AsRawFd,
    panic, str,
    sync::Mutex,
};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not enter raw mode: {}", source))]
    EnterRawMode {
        source: nix::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not initialize signal receiver: {:?}", source))]
    SignalReceiverInit {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not write to terminal: {}", source))]
    TerminalOutput {
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not get window size"))]
    GetWindowSize { backtrace: Backtrace },
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
    pub(crate) fn new() -> Result<Self> {
        use termios::SpecialCharacterIndices::*;

        let stdin = io::stdin();
        let stdout = io::stdout();

        let fd = stdin.as_raw_fd();
        let mut raw = termios::tcgetattr(fd).context(EnterRawMode)?;
        let orig_termios = raw.clone();

        // Set raw mode flags
        termios::cfmakeraw(&mut raw);
        // Set control characters
        raw.control_chars[VMIN as usize] = 0; // minimum number of bytes of input needed before `read()`
        raw.control_chars[VTIME as usize] = 1; // maximum amount of time to wait before `read()` returns

        termios::tcsetattr(fd, SetArg::TCSAFLUSH, &raw).context(EnterRawMode)?;

        {
            let orig_termios = Mutex::new(orig_termios.clone());
            let saved_hook = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                match orig_termios.try_lock() {
                    Err(e) => eprintln!("failed to acquire lock: {}", e),
                    Ok(orig_termios) => {
                        if let Err(e) = termios::tcsetattr(fd, SetArg::TCSAFLUSH, &orig_termios) {
                            eprintln!("failed to reset terminal mode: {}", e);
                        }
                    }
                }
                saved_hook(info);
            }));
        }

        let sigwinch_receiver = SignalReceiver::new_sigwinch().context(SignalReceiverInit)?;

        let mut term = Self {
            stdin,
            stdout,
            screen_size: Size::default(),
            sigwinch_receiver,
            orig_termios,
        };

        term.update_screen_size()?;

        Ok(term)
    }

    pub(crate) fn hide_cursor(&mut self) -> Result<HideCursor> {
        HideCursor::new(io::stdout())
    }

    pub(crate) fn maybe_update_screen_size(&mut self) -> Result<bool> {
        let need_update = self.sigwinch_receiver.received();
        if need_update {
            self.update_screen_size()?;
        }
        Ok(need_update)
    }

    fn update_screen_size(&mut self) -> Result<()> {
        self.screen_size = self.get_window_size()?;
        Ok(())
    }

    fn get_window_size(&mut self) -> Result<Size> {
        if let Some((cols, rows)) = term_size::dimensions() {
            return Ok(Size { cols, rows });
        }
        GetWindowSize.fail()
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        let fd = self.stdin.as_raw_fd();
        termios::tcsetattr(fd, SetArg::TCSAFLUSH, &self.orig_termios)
            .expect("failed to restore terminal mode");
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

pub(crate) struct HideCursor {
    stdout: Stdout,
}

impl HideCursor {
    fn new(mut stdout: Stdout) -> Result<Self> {
        // Hide cursor
        write!(&mut stdout, "\x1b[?25l").context(TerminalOutput)?;

        Ok(HideCursor { stdout })
    }
}

impl Drop for HideCursor {
    fn drop(&mut self) {
        // Show cursor
        write!(&mut self.stdout, "\x1b[?25h").expect("failed to write to terminal");
        self.stdout.flush().expect("failed to flush to stdout");
    }
}
