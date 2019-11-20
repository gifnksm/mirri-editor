use crate::{
    geom::{Point, Segment, Size},
    render::{RenderItem, RenderStrExt},
    syntax::{Highlight, Syntax},
    text_buffer::Status,
};
use itertools::Either;
use std::{iter, path::Path};

#[derive(Debug)]
pub(crate) struct Welcome {
    render_size: Size,
    message: String,
}

impl Welcome {
    pub(crate) fn new(render_size: Size) -> Self {
        Self {
            render_size,
            message: format!(
                "{} -- version {}",
                env!("CARGO_PKG_DESCRIPTION"),
                env!("CARGO_PKG_VERSION")
            ),
        }
    }

    pub(crate) fn status(&self) -> Status {
        Status {
            filename: Some(Path::new("*Welcome*")),
            dirty: false,
            readonly: false,
            cursor: Point::default(),
            lines: 0,
            syntax: Syntax::select(None::<String>),
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.render_size = render_size;
    }

    #[allow(clippy::needless_lifetimes)] // false positive
    pub(crate) fn render_with_highlight<'a>(
        &'a self,
    ) -> impl Iterator<Item = Box<dyn Iterator<Item = (Highlight, RenderItem)> + 'a>> {
        let render_segment = Segment {
            origin: 0,
            size: self.render_size.cols,
        };
        let pre_rows = (self.render_size.rows - 1) / 3;
        let post_rows = self.render_size.rows - pre_rows - 1;

        let tilde_width = "~".render_width(0);
        let message_width = self.message.render_width(tilde_width);
        let message = if tilde_width + message_width >= self.render_size.cols {
            Either::Left(self.message.render_within(0, render_segment))
        } else {
            let padding_size = ((self.render_size.cols - message_width) / 2).saturating_sub(1);
            let tilde = "~".render_within(0, render_segment);
            let padding = iter::once(RenderItem::padding(padding_size));
            let message = self.message.render_within(1, render_segment);
            Either::Right(tilde.chain(padding).chain(message))
        };

        let message_row = iter::repeat(Highlight::Normal).zip(message);
        let empty_row = iter::repeat(Highlight::Normal).zip("~".render_within(0, render_segment));

        let pre = iter::repeat(Either::Left(empty_row.clone())).take(pre_rows);
        let message = iter::once(Either::Right(message_row));
        let post = iter::repeat(Either::Left(empty_row)).take(post_rows);

        pre.chain(message)
            .chain(post)
            .map(|iter| Box::new(iter) as Box<dyn Iterator<Item = (Highlight, RenderItem)>>)
    }
}
