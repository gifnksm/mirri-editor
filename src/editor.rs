use crate::{
    row::Row,
    terminal::{self, RawTerminal},
};
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub(crate) struct Editor {
    pub(crate) cx: usize,
    pub(crate) cy: usize,
    pub(crate) rx: usize,
    pub(crate) screen_cols: usize,
    pub(crate) screen_rows: usize,
    pub(crate) row_off: usize,
    pub(crate) col_off: usize,
    pub(crate) rows: Vec<Row>,
    pub(crate) filename: Option<PathBuf>,
    pub(crate) term: RawTerminal,
}

impl Editor {
    pub(crate) fn new() -> Result<Self> {
        let mut term = RawTerminal::new().context(TerminalError)?;
        let (screen_cols, mut screen_rows) = term.get_window_size().context(TerminalError)?;
        screen_rows -= 1; // status bar height

        Ok(Editor {
            cx: 0,
            cy: 0,
            rx: 0,
            screen_rows,
            screen_cols,
            row_off: 0,
            col_off: 0,
            rows: vec![],
            filename: None,
            term,
        })
    }

    pub(crate) fn append_row(&mut self, s: String) {
        self.rows.push(Row::new(s));
    }
}
