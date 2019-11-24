use crate::{
    geom::{Point, Rect, Segment, Size},
    row::Row,
    syntax::Syntax,
};
use std::{
    cell::{Ref, RefCell},
    ops::Range,
};

#[derive(Debug)]
pub(crate) struct Welcome {
    render_rect: Rect,
    message_row: RefCell<Row>,
    empty_row: RefCell<Row>,
}

impl Welcome {
    pub(crate) fn new(render_size: Size) -> Self {
        let render_rect = Rect {
            origin: Point::default(),
            size: render_size,
        };
        let message = format!(
            "{} -- version {}",
            env!("CARGO_PKG_DESCRIPTION"),
            env!("CARGO_PKG_VERSION")
        );

        let mut message_row = Row::new(message);
        let mut empty_row = Row::new("~");
        let syntax = Syntax::select(None::<&str>);
        message_row.update_highlight(syntax, None, None);
        empty_row.update_highlight(syntax, None, None);

        let mut welcome = Self {
            render_rect,
            message_row: RefCell::new(message_row),
            empty_row: RefCell::new(empty_row),
        };
        welcome.set_render_size(render_size);
        welcome
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.render_rect.size = render_size;
    }

    pub(crate) fn render_rows(&self) -> RenderRows {
        RenderRows {
            welcome: self,
            idx: self.render_rect.y_segment().range(),
            message_idx: (self.render_rect.size.rows - 1) / 3,
            render_rect: self.render_rect,
        }
    }
}

pub(crate) struct RenderRows<'a> {
    welcome: &'a Welcome,
    idx: Range<usize>,
    message_idx: usize,
    render_rect: Rect,
}

impl<'a> Iterator for RenderRows<'a> {
    type Item = Vec<(Segment, Ref<'a, Row>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.idx.next()?;
        let segment = self.render_rect.x_segment();
        if idx == self.message_idx {
            Some(vec![(segment, self.welcome.message_row.borrow())])
        } else {
            Some(vec![(segment, self.welcome.empty_row.borrow())])
        }
    }
}
