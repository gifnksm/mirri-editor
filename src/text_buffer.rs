use crate::{
    editor::CursorMove,
    file,
    geom::{Point, Rect, Size},
    output,
    row::Row,
    syntax::{Highlight, SavedHighlight, Syntax},
    util::SliceExt,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct TextBuffer {
    filename: Option<PathBuf>,
    pub(crate) syntax: &'static Syntax<'static>,
    pub(crate) c: Point,
    pub(crate) rows: Vec<Row>,
    dirty: bool,

    pub(crate) render_rect: Rect,
}

impl TextBuffer {
    pub(crate) fn new(render_size: Size) -> Self {
        let filename = None;
        let syntax = Syntax::select(filename.as_ref());
        Self {
            filename,
            syntax,
            c: Point::default(),
            rows: vec![],
            dirty: false,
            render_rect: Rect {
                origin: Point::default(),
                size: render_size,
            },
        }
    }

    pub(crate) fn from_file(filename: impl Into<PathBuf>, render_rect: Size) -> file::Result<Self> {
        let filename = filename.into();
        let lines = file::open(&filename)?;
        let mut buf = Self::new(render_rect);
        buf.set_filename(Some(filename));
        for line in lines {
            buf.append_row(line);
        }
        buf.dirty = false;
        Ok(buf)
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.render_rect.size = render_size;
    }

    pub(crate) fn scroll(&mut self) -> Point {
        let rx = if let Some(row) = self.rows.get(self.c.y) {
            output::get_render_width(&row.chars[..self.c.x])
        } else {
            0
        };

        if self.render_rect.origin.y > self.c.y {
            self.render_rect.origin.y = self.c.y;
        }
        if self.render_rect.origin.y + (self.render_rect.size.rows - 1) < self.c.y {
            self.render_rect.origin.y = self.c.y - (self.render_rect.size.rows - 1);
        }
        if self.render_rect.origin.x > rx {
            self.render_rect.origin.x = rx;
        }
        if self.render_rect.origin.x + (self.render_rect.size.cols - 1) < rx {
            self.render_rect.origin.x = rx - (self.render_rect.size.cols - 1);
        }
        Point {
            x: rx - self.render_rect.origin.x,
            y: self.c.y - self.render_rect.origin.y,
        }
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
        let row = self.rows.get(self.c.y);
        enum YScroll {
            Up(usize),
            Down(usize),
        }
        let mut y_scroll = None;
        match mv {
            Left => {
                if let Some(ch) = row.and_then(|row| row.chars[..self.c.x].chars().next_back()) {
                    self.c.x -= ch.len_utf8();
                } else if self.c.y > 0 {
                    self.c.y -= 1;
                    self.c.x = self.rows[self.c.y].chars.len();
                }
            }
            Right => {
                if let Some(row) = row {
                    if let Some(ch) = row.chars[self.c.x..].chars().next() {
                        self.c.x += ch.len_utf8();
                    } else {
                        self.c.y += 1;
                        self.c.x = 0;
                    }
                }
            }
            Home => self.c.x = 0,
            End => {
                if let Some(row) = row {
                    self.c.x = row.chars.len();
                }
            }
            Up => y_scroll = Some(YScroll::Up(1)),
            Down => y_scroll = Some(YScroll::Down(1)),
            PageUp => {
                y_scroll = Some(YScroll::Up(
                    (self.c.y - self.render_rect.origin.y) + self.render_rect.size.rows,
                ))
            }
            PageDown => {
                y_scroll = Some(YScroll::Down(
                    (self.render_rect.origin.y + (self.render_rect.size.rows - 1) - self.c.y)
                        + self.render_rect.size.rows,
                ))
            }
        }

        if let Some(scroll) = y_scroll {
            // Adjust cursor x position to the nearest char boundary in rendered texts
            let rx = self
                .rows
                .get(self.c.y)
                .map(|row| output::get_render_width(&row.chars[..self.c.x]))
                .unwrap_or(0);
            match scroll {
                YScroll::Up(dy) => {
                    if self.c.y < dy {
                        self.c.y = 0;
                    } else {
                        self.c.y -= dy;
                    }
                }
                YScroll::Down(dy) => {
                    self.c.y += dy;
                    if self.c.y >= self.rows.len() {
                        self.c.y = self.rows.len();
                    }
                }
            }
            self.c.x = self
                .rows
                .get(self.c.y)
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
        if self.c.y == self.rows.len() {
            self.append_row("".into());
        }
        self.rows[self.c.y].insert_char(self.c.x, ch);
        self.move_cursor(CursorMove::Right);
        self.dirty = true;
    }

    pub(crate) fn insert_newline(&mut self) {
        if let Some(row) = self.rows.get_mut(self.c.y) {
            let rest = row.split(self.c.x);
            self.insert_row(self.c.y + 1, rest);
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
        let (left, right) = self.rows.split_at_mut(self.c.y + 1);
        let cur = left.last_mut().unwrap();
        let next = right.first();
        if self.c.x < cur.chars.len() {
            cur.delete_char(self.c.x);
            self.dirty = true;
        } else if let Some(next) = next {
            cur.append_str(&next.chars);
            self.delete_row(self.c.y + 1);
            self.dirty = true;
        }
    }

    pub(crate) fn find_start(&mut self) -> Find {
        Find {
            saved_c: self.c,
            saved_origin: self.render_rect.origin,
            saved_highlight: None,
            is_forward: true,
            last_match: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Find {
    saved_c: Point,
    saved_origin: Point,
    saved_highlight: Option<(usize, SavedHighlight)>,
    is_forward: bool,
    last_match: Option<(usize, usize, usize)>,
}

impl Find {
    pub(crate) fn execute(&mut self, buffer: &mut TextBuffer, _query: &str) {
        self.restore_highlight(buffer);
    }

    pub(crate) fn cancel(&mut self, buffer: &mut TextBuffer, _query: &str) {
        self.restore_highlight(buffer);

        buffer.c = self.saved_c;
        buffer.render_rect.origin = self.saved_origin;
    }

    pub(crate) fn input(&mut self, buffer: &mut TextBuffer, query: &str) {
        self.restore_highlight(buffer);
        self.last_match = None;
        self.search(buffer, query);
    }

    pub(crate) fn search_forward(&mut self, buffer: &mut TextBuffer, query: &str) {
        self.restore_highlight(buffer);
        self.is_forward = true;
        self.search(buffer, query);
    }

    pub(crate) fn search_backward(&mut self, buffer: &mut TextBuffer, query: &str) {
        self.restore_highlight(buffer);
        self.is_forward = false;
        self.search(buffer, query);
    }

    fn restore_highlight(&mut self, buffer: &mut TextBuffer) {
        if let Some((idx, hl)) = self.saved_highlight.take() {
            buffer.rows[idx].syntax_mut().restore(&hl);
        }
    }

    fn search(&mut self, buffer: &mut TextBuffer, query: &str) {
        let (mut cy, mut cx_s, mut cx_e) = self
            .last_match
            .unwrap_or((buffer.c.y, buffer.c.x, buffer.c.x));

        for _ in 0..=buffer.rows.len() {
            let [prev, row, next] = buffer.rows.get3_mut(cy);
            let row = row.unwrap();
            row.update_syntax(buffer.syntax, prev, next);

            let (idx_off, res) = if self.is_forward {
                (cx_e, row.chars[cx_e..].match_indices(query).next())
            } else {
                (0, row.chars[..cx_s].rmatch_indices(query).next())
            };

            if let Some((dx, s)) = res {
                let cx = idx_off + dx;
                let s_len = s.len();
                self.last_match = Some((cy, cx, cx + s.len()));
                buffer.c.y = cy;
                buffer.c.x = cx;

                let syntax = row.syntax_mut();
                self.saved_highlight = Some((cy, syntax.save(cx, s_len)));
                syntax.overwrite(cx, s_len, Highlight::Match);
                break;
            }

            if self.is_forward {
                cy = (cy + 1) % buffer.rows.len();
            } else if cy == 0 {
                cy = buffer.rows.len() - 1;
            } else {
                cy -= 1;
            }

            let row = &mut buffer.rows[cy];
            cx_s = row.chars.len();
            cx_e = 0;
        }
    }
}
