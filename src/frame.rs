use crate::{
    geom::{Point, Size},
    text_buffer::TextBuffer,
    text_buffer_view::TextBufferView,
};
use std::mem;

#[derive(Debug)]
pub(crate) enum Frame {
    Empty {
        render_size: Size,
    },
    Leaf {
        buffer_view: TextBufferView,
        render_size: Size,
    },
}

impl Frame {
    pub(crate) fn new(render_size: Size) -> Self {
        Frame::Empty { render_size }
    }

    pub(crate) fn set_buffer_view(
        &mut self,
        mut buffer_view: TextBufferView,
    ) -> Option<TextBufferView> {
        let (org, render_size) = match self {
            Self::Empty { render_size } => {
                let render_size = *render_size;
                buffer_view.set_render_size(render_size);
                *self = Self::Leaf {
                    buffer_view,
                    render_size,
                };
                return None;
            }
            Self::Leaf {
                buffer_view: bv,
                render_size,
            } => (bv, *render_size),
        };
        buffer_view.set_render_size(render_size);
        Some(mem::replace(org, buffer_view))
    }

    pub(crate) fn close(&mut self) {
        match self {
            Self::Empty { .. } => {}
            Self::Leaf { render_size, .. } => {
                let render_size = *render_size;
                *self = Self::Empty { render_size }
            }
        }
    }

    pub(crate) fn buffer_view(&self) -> Option<&TextBufferView> {
        match self {
            Self::Empty { .. } => None,
            Self::Leaf { buffer_view, .. } => Some(buffer_view),
        }
    }

    pub(crate) fn buffer_view_mut(&mut self) -> Option<&mut TextBufferView> {
        match self {
            Self::Empty { .. } => None,
            Self::Leaf { buffer_view, .. } => Some(buffer_view),
        }
    }

    pub(crate) fn buffer_view_or_create(&mut self) -> &mut TextBufferView {
        if let Self::Empty { render_size } = self {
            *self = Self::Leaf {
                buffer_view: TextBufferView::new(TextBuffer::new(), *render_size),
                render_size: *render_size,
            };
        }

        match self {
            Self::Empty { .. } => unreachable!(),
            Self::Leaf { buffer_view, .. } => buffer_view,
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        match self {
            Self::Empty { render_size: rs } => {
                *rs = render_size;
            }
            Self::Leaf {
                buffer_view,
                render_size: rs,
            } => {
                *rs = render_size;
                buffer_view.set_render_size(render_size);
            }
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        match self {
            Self::Empty { .. } => Point::default(),
            Self::Leaf { buffer_view, .. } => buffer_view.scroll(),
        }
    }

    pub(crate) fn update_highlight(&mut self) {
        match self {
            Self::Empty { .. } => {}
            Self::Leaf { buffer_view, .. } => buffer_view.update_highlight(),
        }
    }
}
