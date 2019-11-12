use crate::{
    file, input,
    row::Row,
    syntax::Syntax,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum CursorMove {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug)]
pub(crate) struct Editor {
    pub(crate) cx: usize,
    pub(crate) cy: usize,
    pub(crate) screen_cols: usize,
    pub(crate) screen_rows: usize,
    pub(crate) row_off: usize,
    pub(crate) col_off: usize,
    pub(crate) rows: Vec<Row>,
    pub(crate) dirty: bool,
    pub(crate) quit_times: usize,
    pub(crate) filename: Option<PathBuf>,
    pub(crate) status_msg: Option<(Instant, String)>,
    pub(crate) syntax: &'static Syntax<'static>,
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
            screen_rows,
            screen_cols,
            row_off: 0,
            col_off: 0,
            rows: vec![],
            dirty: false,
            quit_times: QUIT_TIMES,
            filename: None,
            status_msg: None,
            syntax: Syntax::select(None::<&str>),
            term,
        })
    }

    fn set_filename(&mut self, filename: Option<PathBuf>) {
        self.filename = filename;
        self.syntax = Syntax::select(self.filename.as_ref());
        for row in &mut self.rows {
            row.invalidate_syntax();
        }
    }

    pub(crate) fn open(&mut self, filename: impl Into<PathBuf>) {
        let filename = filename.into();
        match file::open(&filename) {
            Ok(lines) => {
                for line in lines {
                    self.append_row(line);
                }
                self.set_filename(Some(filename));
                self.dirty = false;
            }
            Err(e) => {
                self.set_status_msg(format!("{}", e));
            }
        }
    }

    pub(crate) fn save(&mut self) -> input::Result<()> {
        let filename = if let Some(filename) = &self.filename {
            filename.clone()
        } else if let Some(filename) =
            input::prompt(self, "Save as: {} (ESC to cancel)")?.map(Into::into)
        {
            filename
        } else {
            self.set_status_msg("Save aborted");
            return Ok(());
        };

        let lines = self.rows.iter().map(|row| &row.chars);
        match file::save(&filename, lines) {
            Ok(bytes) => {
                self.set_status_msg(format!("{} bytes written to disk", bytes));
                self.set_filename(Some(filename));
                self.dirty = false;
            }
            Err(e) => {
                self.set_status_msg(format!("Can't save! {}", e));
            }
        }

        Ok(())
    }

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        use CursorMove::*;
        let row = self.rows.get(self.cy);
        match mv {
            Left => {
                if let Some(ch) = row.and_then(|row| row.chars[..self.cx].chars().next_back()) {
                    self.cx -= ch.len_utf8();
                } else if self.cy > 0 {
                    self.cy -= 1;
                    self.cx = self.rows[self.cy].chars.len();
                }
            }
            Right => {
                if let Some(row) = row {
                    if let Some(ch) = row.chars[self.cx..].chars().next() {
                        self.cx += ch.len_utf8();
                    } else {
                        self.cy += 1;
                        self.cx = 0;
                    }
                }
            }
            Up => {
                if self.cy > 0 {
                    self.cy -= 1
                }
            }
            Down => {
                if self.cy + 1 < self.rows.len() {
                    self.cy += 1
                }
            }
            Home => self.cx = 0,
            End => {
                if let Some(row) = row {
                    self.cx = row.chars.len();
                }
            }
            PageUp => {
                self.cy = self.row_off;
                for _ in 0..self.screen_rows {
                    self.move_cursor(CursorMove::Up);
                }
            }
            PageDown => {
                self.cy = self.row_off + self.screen_rows - 1;
                if self.cy + 1 > self.rows.len() {
                    if self.rows.is_empty() {
                        self.cy = 0;
                    } else {
                        self.cy = self.rows.len() - 1;
                    }
                }
                for _ in 0..self.screen_rows {
                    self.move_cursor(CursorMove::Down);
                }
            }
        }

        let row = self.rows.get(self.cy);
        let row_len = row.map(|r| r.chars.len()).unwrap_or(0);
        if self.cx > row_len {
            self.cx = row_len;
        }
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
        self.move_cursor(CursorMove::Right);
        self.dirty = true;
    }

    pub(crate) fn insert_newline(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cy) {
            let rest = row.split(self.cx);
            self.insert_row(self.cy + 1, rest);
        } else {
            self.append_row("".into());
        }
        self.move_cursor(CursorMove::Right);
        self.dirty = true;
    }

    pub(crate) fn delete_back_char(&mut self) {
        self.move_cursor(CursorMove::Left);
        self.delete_char();
    }

    pub(crate) fn delete_char(&mut self) {
        let (left, right) = self.rows.split_at_mut(self.cy + 1);
        let cur = left.last_mut().unwrap();
        let next = right.first();
        if self.cx < cur.chars.len() {
            cur.delete_char(self.cx);
            self.dirty = true;
        } else if let Some(next) = next {
            cur.append_str(&next.chars);
            self.delete_row(self.cy + 1);
            self.dirty = true;
        }
    }
}
