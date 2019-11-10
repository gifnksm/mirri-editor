use crate::{
    row::Row,
    terminal::{self, RawTerminal},
};
use snafu::{ResultExt, Snafu};
use std::{path::PathBuf, time::Instant};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) const QUIT_TIMES: usize = 3;

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
    pub(crate) dirty: bool,
    pub(crate) quit_times: usize,
    pub(crate) filename: Option<PathBuf>,
    pub(crate) status_msg: Option<(Instant, String)>,
    pub(crate) term: RawTerminal,
}

impl Editor {
    pub(crate) fn new() -> Result<Self> {
        let mut term = RawTerminal::new().context(TerminalError)?;
        let (screen_cols, mut screen_rows) = term.get_window_size().context(TerminalError)?;
        screen_rows -= 2; // status bar height + message bar height

        Ok(Editor {
            cx: 0,
            cy: 0,
            rx: 0,
            screen_rows,
            screen_cols,
            row_off: 0,
            col_off: 0,
            rows: vec![],
            dirty: false,
            quit_times: QUIT_TIMES,
            filename: None,
            status_msg: None,
            term,
        })
    }

    pub(crate) fn append_row(&mut self, s: String) {
        self.rows.push(Row::new(s));
        self.dirty = true;
    }

    pub(crate) fn set_status_msg(&mut self, s: impl Into<String>) {
        let now = Instant::now();
        self.status_msg = Some((now, s.into()));
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        if self.cy == self.rows.len() {
            self.append_row("".into());
        }
        self.rows[self.cy].insert_char(self.cx, ch);
        self.cx += 1;
        self.dirty = true;
    }

    pub(crate) fn delete_char(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cy) {
            if self.cx > 0 {
                row.delete_char(self.cx - 1);
                self.cx -= 1;
                self.dirty = true;
            }
        }
    }
}
