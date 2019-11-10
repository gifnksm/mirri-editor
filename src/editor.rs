use crate::{
    file, input,
    row::Row,
    terminal::{self, RawTerminal},
};
use snafu::{ResultExt, Snafu};
use std::{path::PathBuf, time::Instant};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
    #[snafu(display("{}", source))]
    InputError { source: input::Error },
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

    pub(crate) fn save(&mut self) -> input::Result<()> {
        if self.filename.is_none() {
            self.filename = input::prompt(self, "Save as: {} (ESC to cancel)")?.map(Into::into);
        }
        let filename = if let Some(filename) = self.filename.as_ref() {
            filename
        } else {
            self.set_status_msg("Save aborted");
            return Ok(());
        };

        let lines = self.rows.iter().map(|row| &row.chars);
        match file::save(filename, lines) {
            Ok(bytes) => {
                self.dirty = false;
                self.set_status_msg(format!("{} bytes written to disk", bytes));
            }
            Err(e) => {
                self.set_status_msg(format!("Can't save! {}", e));
            }
        }

        Ok(())
    }

    pub(crate) fn insert_row(&mut self, at: usize, s: String) {
        self.rows.insert(at, Row::new(s));
        self.dirty = true;
    }

    pub(crate) fn append_row(&mut self, s: String) {
        self.insert_row(self.rows.len(), s);
    }

    pub(crate) fn delete_row(&mut self, at: usize) {
        self.rows.remove(at);
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

    pub(crate) fn insert_newline(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cy) {
            let rest = row.split(self.cx);
            self.insert_row(self.cy + 1, rest);
        } else {
            self.append_row("".into());
        }
        self.cy += 1;
        self.cx = 0;
        self.dirty = true;
    }

    pub(crate) fn delete_char(&mut self) {
        if self.cx > 0 {
            if let Some(row) = self.rows.get_mut(self.cy) {
                row.delete_char(self.cx - 1);
                self.cx -= 1;
                self.dirty = true;
            }
        } else {
            let (left, right) = self.rows.split_at_mut(self.cy);
            if let (Some(prev), Some(row)) = (left.last_mut(), right.first_mut()) {
                self.cx = prev.chars.len();
                prev.append_str(&row.chars);
                self.delete_row(self.cy);
                self.cy -= 1;
                self.dirty = true;
            }
        }
    }
}
