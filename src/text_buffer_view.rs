use crate::{
    editor::CursorMove,
    geom::{Point, Rect, Size},
    render::RenderItem,
    syntax::Highlight,
    text_buffer::{Status, TextBuffer},
};

#[derive(Debug, Clone)]
pub(crate) struct TextBufferView {
    buffer: TextBuffer,
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
            buffer,
            c: Point::default(),
            render_rect,
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.render_rect.size = render_size;
    }

    pub(crate) fn update_highlight(&mut self) {
        self.buffer.update_highlight(self.render_rect)
    }

    pub(crate) fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub(crate) fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    pub(crate) fn status(&self) -> Status {
        self.buffer.status(self.c)
    }

    #[allow(clippy::needless_lifetimes)] // false positive
    pub(crate) fn render_with_highlight<'a>(
        &'a self,
    ) -> impl Iterator<Item = Box<dyn Iterator<Item = (Highlight, RenderItem)> + 'a>> {
        self.buffer.render_with_highlight(self.render_rect)
    }

    pub(crate) fn scroll(&mut self) -> Point {
        let rx = self
            .buffer
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
        let row = &self.buffer.rows()[self.c.y];
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
                    self.c.x = self.buffer.rows()[self.c.y].chars().len();
                }
            }
            Right => {
                if let Some(ch) = row.chars()[self.c.x..].chars().next() {
                    self.c.x += ch.len_utf8();
                } else if self.c.y < self.buffer.rows().len() - 1 {
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
                self.c.y = self.buffer.rows().len() - 1;
                self.c.x = self.buffer.rows()[self.c.y].chars().len();
            }
        }

        if let Some(scroll) = y_scroll {
            // Adjust cursor x position to the nearest char boundary in rendered texts
            let rx = self.buffer.rows()[self.c.y].get_rx_from_cx(self.c.x);
            match scroll {
                YScroll::Up(dy) => self.c.y = self.c.y.saturating_sub(dy),
                YScroll::Down(dy) => {
                    self.c.y += dy;
                    let max_y = self.buffer.rows().len() - 1;
                    if self.c.y >= max_y {
                        self.c.y = max_y;
                    }
                }
            }
            self.c.x = self.buffer.rows()[self.c.y].get_cx_from_rx(rx);
        }
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.buffer.insert_char(self.c, ch);
        self.move_cursor(CursorMove::Right);
    }

    pub(crate) fn insert_newline(&mut self) {
        self.buffer.insert_newline(self.c);
        self.move_cursor(CursorMove::Right);
    }

    pub(crate) fn delete_back_char(&mut self) {
        self.move_cursor(CursorMove::Left);
        self.delete_char();
    }

    pub(crate) fn delete_char(&mut self) {
        self.buffer.delete_char(self.c);
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
            buffer_view.buffer.rows_mut()[idx]
                .syntax_mut()
                .clear_overlay();
        }
    }

    fn search(&mut self, buffer_view: &mut TextBufferView, query: &str) {
        let (mut cy, mut cx_s, mut cx_e) =
            self.last_match
                .unwrap_or((buffer_view.c.y, buffer_view.c.x, buffer_view.c.x));

        for _ in 0..=buffer_view.buffer.rows().len() {
            let row = &mut buffer_view.buffer.rows_mut()[cy];

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
                cy = (cy + 1) % buffer_view.buffer.rows().len();
            } else if cy == 0 {
                cy = buffer_view.buffer.rows().len() - 1;
            } else {
                cy -= 1;
            }

            let row = &mut buffer_view.buffer.rows_mut()[cy];
            cx_s = row.chars().len();
            cx_e = 0;
        }
    }
}
