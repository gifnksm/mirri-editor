use crate::{
    file,
    geom::{Point, Rect, Segment},
    render::RenderItem,
    row::Row,
    syntax::{Highlight, Syntax},
    util::SliceExt,
};
use std::{
    iter,
    path::{Path, PathBuf},
    usize,
};

#[derive(Debug, Clone)]
pub(crate) struct TextBuffer {
    filename: Option<PathBuf>,
    syntax: &'static Syntax<'static>,
    rows: Vec<Row>,
    dirty: bool,
    readonly: bool,
    empty_row: Row,
}

impl TextBuffer {
    fn new_empty() -> Self {
        let filename = None;
        let syntax = Syntax::select(filename.as_ref());
        let mut empty_row = Row::new("~");
        empty_row
            .syntax_mut()
            .set_overlay(0..1, Highlight::LineMarker);

        Self {
            filename,
            syntax,
            rows: vec![],
            dirty: false,
            readonly: false,
            empty_row,
        }
    }

    pub(crate) fn new() -> Self {
        let mut buf = Self::new_empty();
        buf.append_row("");
        buf
    }

    pub(crate) fn from_file(filename: impl Into<PathBuf>) -> file::Result<Self> {
        let filename = filename.into();
        let mut buf = Self::new_empty();
        if file::exists(&filename) {
            buf.readonly = !file::writable(&filename)?;
            let lines = file::open(&filename)?;
            for line in lines {
                buf.append_row(line);
            }
        } else {
            buf.append_row("");
        }
        buf.dirty = false;
        buf.set_filename(Some(filename));
        Ok(buf)
    }

    pub(crate) fn dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn readonly(&self) -> bool {
        self.readonly
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) fn rows_mut(&mut self) -> &mut [Row] {
        &mut self.rows
    }

    pub(crate) fn status(&self, c: Point) -> Status {
        Status {
            filename: self.filename.as_ref().map(|p| p.as_ref()),
            dirty: self.dirty,
            readonly: self.readonly,
            cursor: c,
            lines: self.rows.len(),
            syntax: self.syntax,
        }
    }

    #[allow(clippy::needless_lifetimes)] // false positive
    pub(crate) fn render_with_highlight<'a>(
        &'a self,
        render_rect: Rect,
    ) -> impl Iterator<Item = Box<dyn Iterator<Item = (Highlight, RenderItem)> + 'a>> {
        let row_render_segment = render_rect.x_segment();
        let empty_render_segment = Segment {
            origin: 0,
            size: render_rect.size.cols,
        };
        self.rows[render_rect.origin.y..]
            .iter()
            .map(move |row| row.render_with_highlight(row_render_segment))
            .chain(
                iter::repeat(&self.empty_row)
                    .map(move |row| row.render_with_highlight(empty_render_segment)),
            )
            .take(render_rect.size.rows)
            .map(|iter| Box::new(iter) as Box<dyn Iterator<Item = (Highlight, RenderItem)>>)
    }

    pub(crate) fn update_highlight(&mut self, render_rect: Rect) {
        for cy in 0..render_rect.origin.y + render_rect.size.rows {
            let [prev, row, next] = self.rows.get3_mut(cy);
            if let Some(row) = row {
                row.update_highlight(self.syntax, prev, next);
            }
        }
        self.empty_row.update_highlight(self.syntax, None, None);
    }

    pub(crate) fn save(&mut self) -> file::Result<usize> {
        let filename = self.filename.as_ref().unwrap();
        let lines = self.rows.iter().map(|row| row.chars());
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

    fn insert_row(&mut self, at: usize, s: String) {
        self.rows.insert(at, Row::new(s));
        self.dirty = true;
    }

    fn append_row(&mut self, s: impl Into<String>) {
        self.insert_row(self.rows.len(), s.into());
    }

    fn delete_row(&mut self, at: usize) {
        self.rows.remove(at);
        self.dirty = true;
    }

    pub(crate) fn insert_char(&mut self, c: Point, ch: char) {
        self.rows[c.y].insert_char(c.x, ch);
        self.dirty = true;
    }

    pub(crate) fn insert_newline(&mut self, c: Point) {
        let rest = self.rows[c.y].split(c.x);
        self.insert_row(c.y + 1, rest);
        self.dirty = true;
    }

    pub(crate) fn delete_char(&mut self, c: Point) {
        let (left, right) = self.rows.split_at_mut(c.y + 1);
        let cur = left.last_mut().unwrap();
        let next = right.first();
        if c.x < cur.chars().len() {
            cur.delete_char(c.x);
            self.dirty = true;
        } else if let Some(next) = next {
            cur.append_str(&next.chars());
            self.delete_row(c.y + 1);
            self.dirty = true;
        }
    }
}

#[derive(Debug)]
pub(crate) struct Status<'a> {
    pub(crate) filename: Option<&'a Path>,
    pub(crate) dirty: bool,
    pub(crate) readonly: bool,
    pub(crate) cursor: Point,
    pub(crate) lines: usize,
    pub(crate) syntax: &'a Syntax<'a>,
}
