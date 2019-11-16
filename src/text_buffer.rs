use crate::{editor::CursorMove, file, output, row::Row, syntax::Syntax};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct TextBuffer {
    filename: Option<PathBuf>,
    pub(crate) syntax: &'static Syntax<'static>,
    pub(crate) cx: usize,
    pub(crate) cy: usize,
    pub(crate) rows: Vec<Row>,
    dirty: bool,

    pub(crate) row_off: usize,
    pub(crate) col_off: usize,
    pub(crate) render_rows: usize,
    pub(crate) render_cols: usize,
}

impl TextBuffer {
    pub(crate) fn new(render_rows: usize, render_cols: usize) -> Self {
        let filename = None;
        let syntax = Syntax::select(filename.as_ref());
        Self {
            filename,
            syntax,
            cx: 0,
            cy: 0,
            rows: vec![],
            dirty: false,
            row_off: 0,
            col_off: 0,
            render_rows,
            render_cols,
        }
    }

    pub(crate) fn from_file(
        filename: impl Into<PathBuf>,
        render_rows: usize,
        render_cols: usize,
    ) -> file::Result<Self> {
        let filename = filename.into();
        let lines = file::open(&filename)?;
        let mut buf = Self::new(render_rows, render_cols);
        buf.set_filename(Some(filename));
        for line in lines {
            buf.append_row(line);
        }
        Ok(buf)
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn set_render_size(&mut self, render_rows: usize, render_cols: usize) {
        self.render_rows = render_rows;
        self.render_cols = render_cols;
    }

    pub(crate) fn scroll(&mut self) -> (usize, usize) {
        let rx = if let Some(row) = self.rows.get(self.cy) {
            output::get_render_width(&row.chars[..self.cx])
        } else {
            0
        };

        if self.row_off > self.cy {
            self.row_off = self.cy;
        }
        if self.row_off + (self.render_rows - 1) < self.cy {
            self.row_off = self.cy - (self.render_rows - 1);
        }
        if self.col_off > rx {
            self.col_off = rx;
        }
        if self.col_off + (self.render_cols - 1) < rx {
            self.col_off = rx - (self.render_cols - 1);
        }
        (self.cy - self.row_off, rx - self.col_off)
    }

    pub(crate) fn save(&mut self) -> file::Result<usize> {
        let filename = self.filename.as_ref().unwrap();
        let lines = self.rows.iter().map(|row| &row.chars);
        let bytes = file::save(&filename, lines)?;
        self.dirty = false;
        Ok(bytes)
    }

    pub(crate) fn filename(&self) -> Option<&Path> {
        self.filename.as_ref().map(|p| p.as_ref())
    }

    pub(crate) fn set_filename(&mut self, filename: Option<PathBuf>) {
        self.filename = filename;
        self.syntax = Syntax::select(self.filename.as_ref());
        for row in &mut self.rows {
            row.invalidate_syntax();
        }
    }

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        use CursorMove::*;
        let row = self.rows.get(self.cy);
        enum YScroll {
            Up(usize),
            Down(usize),
        }
        let mut y_scroll = None;
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
            Home => self.cx = 0,
            End => {
                if let Some(row) = row {
                    self.cx = row.chars.len();
                }
            }
            Up => y_scroll = Some(YScroll::Up(1)),
            Down => y_scroll = Some(YScroll::Down(1)),
            PageUp => y_scroll = Some(YScroll::Up((self.cy - self.row_off) + self.render_rows)),
            PageDown => {
                y_scroll = Some(YScroll::Down(
                    (self.row_off + (self.render_rows - 1) - self.cy) + self.render_rows,
                ))
            }
        }

        if let Some(scroll) = y_scroll {
            // Adjust cursor x position to the nearest char boundary in rendered texts
            let rx = self
                .rows
                .get(self.cy)
                .map(|row| output::get_render_width(&row.chars[..self.cx]))
                .unwrap_or(0);
            match scroll {
                YScroll::Up(dy) => {
                    if self.cy < dy {
                        self.cy = 0;
                    } else {
                        self.cy -= dy;
                    }
                }
                YScroll::Down(dy) => {
                    self.cy += dy;
                    if self.cy >= self.rows.len() {
                        self.cy = self.rows.len();
                    }
                }
            }
            self.cx = self
                .rows
                .get(self.cy)
                .map(|row| output::get_cx_from_rx(&row.chars, rx))
                .unwrap_or(0);
        }
    }

    fn insert_row(&mut self, at: usize, s: String) {
        self.rows.insert(at, Row::new(s));
        self.dirty = true;
    }

    fn append_row(&mut self, s: String) {
        self.insert_row(self.rows.len(), s);
    }

    fn delete_row(&mut self, at: usize) {
        self.rows.remove(at);
        self.dirty = true;
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
