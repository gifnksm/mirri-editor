use crate::terminal::{self, RawTerminal};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub(crate) struct Editor {
    /// Cursor x position
    pub(crate) cx: usize,
    /// Cursor y position
    pub(crate) cy: usize,
    pub(crate) screen_cols: usize,
    pub(crate) screen_rows: usize,
    pub(crate) term: RawTerminal,
}

impl Editor {
    pub(crate) fn new() -> Result<Self> {
        let mut term = RawTerminal::new().context(TerminalError)?;
        let (screen_cols, screen_rows) = term.get_window_size().context(TerminalError)?;

        Ok(Editor {
            cx: 0,
            cy: 0,
            screen_rows,
            screen_cols,
            term,
        })
    }
}
