use crate::{
    editor::CursorMove,
    geom::{Point, Rect, Segment, Size},
    row::Row,
    syntax::{Highlight, Syntax},
    text_buffer::TextBuffer,
};
use std::{
    cell::{Ref, RefCell, RefMut},
    ops::Range,
    path::Path,
    rc::Rc,
};

#[derive(Debug, Clone)]
pub(crate) struct TextBufferView {
    buffer: Rc<RefCell<TextBuffer>>,
    c: Point,
    render_rect: Rect,
}

impl TextBufferView {
    pub(crate) fn new(buffer: TextBuffer, render_size: Size) -> Self {
        let render_rect = Rect {
            origin: Point::default(),
            size: render_size,
        };
        Self {
            buffer: Rc::new(RefCell::new(buffer)),
            c: Point::default(),
            render_rect,
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.render_rect.size = render_size;
    }

    pub(crate) fn update_highlight(&mut self) {
        self.buffer.borrow_mut().update_highlight(self.render_rect)
    }

    pub(crate) fn buffer(&self) -> Ref<TextBuffer> {
        self.buffer.borrow()
    }

    pub(crate) fn buffer_mut(&mut self) -> RefMut<TextBuffer> {
        self.buffer.borrow_mut()
    }

    pub(crate) fn status(&self) -> Status {
        let buffer = self.buffer.borrow();
        Status {
            filename: ref_filter_map::ref_filter_map(self.buffer.borrow(), |b| b.filename()),
            dirty: buffer.dirty(),
            readonly: buffer.readonly(),
            cursor: self.c,
            lines: buffer.lines(),
            syntax: Ref::map(buffer, |b| b.syntax()),
        }
    }

    pub(crate) fn render_rows(&self) -> RenderRows {
        RenderRows {
            buffer_view: self,
            idx: self.render_rect.y_segment().range(),
            render_rect: self.render_rect,
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        let rx = self
            .buffer
            .borrow()
            .rows()
            .get(self.c.y)
            .map(|row| row.get_rx_from_cx(self.c.x))
            .unwrap_or(0);

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

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        use CursorMove::*;
        let buffer = self.buffer.borrow();
        let row = &buffer.rows()[self.c.y];
        enum YScroll {
            Up(usize),
            Down(usize),
        }
        let mut y_scroll = None;
        match mv {
            Left => {
                if let Some(ch) = row.chars()[..self.c.x].chars().next_back() {
                    self.c.x -= ch.len_utf8();
                } else if self.c.y > 0 {
                    self.c.y -= 1;
                    self.c.x = buffer.rows()[self.c.y].chars().len();
                }
            }
            Right => {
                if let Some(ch) = row.chars()[self.c.x..].chars().next() {
                    self.c.x += ch.len_utf8();
                } else if self.c.y < buffer.rows().len() - 1 {
                    self.c.y += 1;
                    self.c.x = 0;
                }
            }
            Home => self.c.x = 0,
            End => self.c.x = row.chars().len(),
            Up => y_scroll = Some(YScroll::Up(1)),
            Down => y_scroll = Some(YScroll::Down(1)),
            PageUp => {
                y_scroll = Some(YScroll::Up(
                    self.c.y + self.render_rect.size.rows - self.render_rect.origin.y,
                ))
            }
            PageDown => {
                y_scroll = Some(YScroll::Down(
                    (self.render_rect.origin.y + (self.render_rect.size.rows - 1) - self.c.y)
                        + self.render_rect.size.rows,
                ))
            }
            BufferHome => {
                self.c.x = 0;
                self.c.y = 0;
            }
            BufferEnd => {
                self.c.y = buffer.rows().len() - 1;
                self.c.x = buffer.rows()[self.c.y].chars().len();
            }
        }

        if let Some(scroll) = y_scroll {
            // Adjust cursor x position to the nearest char boundary in rendered texts
            let rx = buffer.rows()[self.c.y].get_rx_from_cx(self.c.x);
            match scroll {
                YScroll::Up(dy) => self.c.y = self.c.y.saturating_sub(dy),
                YScroll::Down(dy) => {
                    self.c.y += dy;
                    let max_y = buffer.rows().len() - 1;
                    if self.c.y >= max_y {
                        self.c.y = max_y;
                    }
                }
            }
            self.c.x = buffer.rows()[self.c.y].get_cx_from_rx(rx);
        }
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.buffer.borrow_mut().insert_char(self.c, ch);
        self.move_cursor(CursorMove::Right);
    }

    pub(crate) fn insert_newline(&mut self) {
        self.buffer.borrow_mut().insert_newline(self.c);
        self.move_cursor(CursorMove::Right);
    }

    pub(crate) fn delete_back_char(&mut self) {
        self.move_cursor(CursorMove::Left);
        self.delete_char();
    }

    pub(crate) fn delete_char(&mut self) {
        self.buffer.borrow_mut().delete_char(self.c);
    }

    pub(crate) fn find_start(&mut self) -> Find {
        Find {
            saved_c: self.c,
            saved_highlight: None,
            is_forward: true,
            last_match: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Find {
    saved_c: Point,
    saved_highlight: Option<usize>,
    is_forward: bool,
    last_match: Option<(usize, usize, usize)>,
}

impl Find {
    pub(crate) fn execute(&mut self, buffer_view: &mut TextBufferView, _query: &str) {
        self.restore_highlight(buffer_view);
    }

    pub(crate) fn cancel(&mut self, buffer_view: &mut TextBufferView, _query: &str) {
        self.restore_highlight(buffer_view);

        buffer_view.c = self.saved_c;
    }

    pub(crate) fn input(&mut self, buffer_view: &mut TextBufferView, query: &str) {
        self.restore_highlight(buffer_view);
        self.last_match = None;
        self.search(buffer_view, query);
    }

    pub(crate) fn search_forward(&mut self, buffer_view: &mut TextBufferView, query: &str) {
        self.restore_highlight(buffer_view);
        self.is_forward = true;
        self.search(buffer_view, query);
    }

    pub(crate) fn search_backward(&mut self, buffer_view: &mut TextBufferView, query: &str) {
        self.restore_highlight(buffer_view);
        self.is_forward = false;
        self.search(buffer_view, query);
    }

    fn restore_highlight(&mut self, buffer_view: &mut TextBufferView) {
        if let Some(idx) = self.saved_highlight.take() {
            buffer_view.buffer.borrow_mut().rows_mut()[idx]
                .syntax_mut()
                .clear_overlay();
        }
    }

    fn search(&mut self, buffer_view: &mut TextBufferView, query: &str) {
        let (mut cy, mut cx_s, mut cx_e) =
            self.last_match
                .unwrap_or((buffer_view.c.y, buffer_view.c.x, buffer_view.c.x));

        let mut buffer = buffer_view.buffer.borrow_mut();
        for _ in 0..=buffer.rows().len() {
            let row = &mut buffer.rows_mut()[cy];

            let (idx_off, res) = if self.is_forward {
                (cx_e, row.chars()[cx_e..].match_indices(query).next())
            } else {
                (0, row.chars()[..cx_s].rmatch_indices(query).next())
            };

            if let Some((dx, s)) = res {
                let cx = idx_off + dx;
                let s_len = s.len();
                self.last_match = Some((cy, cx, cx + s.len()));
                buffer_view.c.y = cy;
                buffer_view.c.x = cx;

                row.syntax_mut()
                    .set_overlay(cx..cx + s_len, Highlight::Match);
                self.saved_highlight = Some(cy);
                break;
            }

            if self.is_forward {
                cy = (cy + 1) % buffer.rows().len();
            } else if cy == 0 {
                cy = buffer.rows().len() - 1;
            } else {
                cy -= 1;
            }

            let row = &buffer.rows()[cy];
            cx_s = row.chars().len();
            cx_e = 0;
        }
    }
}

#[derive(Debug)]
pub(crate) struct Status<'a> {
    pub(crate) filename: Option<Ref<'a, Path>>,
    pub(crate) dirty: bool,
    pub(crate) readonly: bool,
    pub(crate) cursor: Point,
    pub(crate) lines: usize,
    pub(crate) syntax: Ref<'a, Syntax<'a>>,
}

pub(crate) struct RenderRows<'a> {
    buffer_view: &'a TextBufferView,
    idx: Range<usize>,
    render_rect: Rect,
}

impl<'a> Iterator for RenderRows<'a> {
    type Item = (Segment, Ref<'a, Row>);

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.idx.next()?;
        let row = Ref::map(self.buffer_view.buffer.borrow(), |b| b.row_at(idx));
        Some((self.render_rect.x_segment(), row))
    }
}
