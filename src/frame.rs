use crate::{
    geom::{Point, Segment, Size},
    row::Row,
    text_buffer::TextBuffer,
    text_buffer_view::TextBufferView,
};
use std::{cell::Ref, mem, ops::Range};

#[derive(Debug, Copy, Clone)]
pub(crate) enum SplitOrientation {
    Vertical,
}

#[derive(Debug)]
pub(crate) enum Frame {
    Empty {
        render_size: Size,
    },
    Leaf {
        buffer_view: TextBufferView,
        render_size: Size,
    },
    Split {
        frames: Vec<Frame>,
        focus_idx: usize,
        orientation: SplitOrientation,
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
        match self {
            Self::Empty { render_size } => {
                let render_size = *render_size;
                buffer_view.set_render_size(render_size);
                *self = Self::Leaf {
                    buffer_view,
                    render_size,
                };
                None
            }
            Self::Leaf {
                buffer_view: org,
                render_size,
            } => {
                buffer_view.set_render_size(*render_size);
                Some(mem::replace(org, buffer_view))
            }
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].set_buffer_view(buffer_view),
        }
    }

    fn render_size(&self) -> Size {
        match self {
            Self::Empty { render_size } => *render_size,
            Self::Leaf { render_size, .. } => *render_size,
            Self::Split { render_size, .. } => *render_size,
        }
    }

    pub(crate) fn close(&mut self) {
        match self {
            Self::Empty { .. } => {}
            Self::Leaf { render_size, .. } => {
                let render_size = *render_size;
                *self = Self::Empty { render_size }
            }
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].close(),
        }
    }

    pub(crate) fn buffer_view(&self) -> Option<&TextBufferView> {
        match self {
            Self::Empty { .. } => None,
            Self::Leaf { buffer_view, .. } => Some(buffer_view),
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].buffer_view(),
        }
    }

    pub(crate) fn buffer_view_mut(&mut self) -> Option<&mut TextBufferView> {
        match self {
            Self::Empty { .. } => None,
            Self::Leaf { buffer_view, .. } => Some(buffer_view),
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].buffer_view_mut(),
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
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].buffer_view_or_create(),
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
            Self::Split {
                frames,
                orientation: SplitOrientation::Vertical,
                render_size: rs,
                ..
            } => {
                *rs = render_size;
                // TODO: reserve ratio of each sub frames
                let base_rows = render_size.rows / frames.len();
                let rem_frames = render_size.rows - base_rows * frames.len();
                for (i, frame) in frames.iter_mut().enumerate() {
                    if i < rem_frames {
                        frame.set_render_size(Size {
                            rows: base_rows + 1,
                            cols: render_size.cols,
                        });
                    } else {
                        frame.set_render_size(Size {
                            rows: base_rows,
                            cols: render_size.cols,
                        });
                    }
                }
            }
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        match self {
            Self::Empty { .. } => Point::default(),
            Self::Leaf { buffer_view, .. } => buffer_view.scroll(),
            Self::Split {
                frames, focus_idx, ..
            } => {
                let mut point = Point::default();
                for (idx, frame) in frames.iter_mut().enumerate() {
                    let p = frame.scroll();
                    if idx == *focus_idx {
                        point = p;
                    }
                }
                point
            }
        }
    }

    pub(crate) fn update_highlight(&mut self) {
        match self {
            Self::Empty { .. } => {}
            Self::Leaf { buffer_view, .. } => buffer_view.update_highlight(),
            Self::Split { frames, .. } => {
                for frame in frames {
                    frame.update_highlight();
                }
            }
        }
    }

    pub(crate) fn render_rows(&self) -> RenderRows {
        RenderRows {
            frame: self,
            ry: 0..self.render_size().rows,
        }
    }

    pub(crate) fn split(&mut self, orientation: SplitOrientation) {
        match self {
            Self::Empty { render_size } => {
                let render_size = *render_size;
                let (size1, size2) = split_size(render_size, orientation);
                let frame1 = Frame::new(size1);
                let frame2 = Frame::new(size2);
                *self = Self::Split {
                    frames: vec![frame1, frame2],
                    focus_idx: 0,
                    orientation,
                    render_size,
                };
            }
            Self::Leaf {
                buffer_view,
                render_size,
            } => {
                let render_size = *render_size;
                let (size1, size2) = split_size(render_size, orientation);
                let mut bv1 = buffer_view.clone();
                let mut bv2 = buffer_view.clone();
                bv1.set_render_size(size1);
                bv2.set_render_size(size2);
                let frame1 = Frame::Leaf {
                    buffer_view: bv1,
                    render_size: size1,
                };
                let frame2 = Frame::Leaf {
                    buffer_view: bv2,
                    render_size: size2,
                };
                *self = Self::Split {
                    frames: vec![frame1, frame2],
                    focus_idx: 0,
                    orientation,
                    render_size,
                };
            }
            Self::Split {
                frames, focus_idx, ..
            } => frames[*focus_idx].split(orientation),
        }
    }

    fn push_render_rows_at<'a>(&'a self, ry: usize, rows: &mut Vec<(Segment, Ref<'a, Row>)>) {
        match self {
            Self::Empty { .. } => {}
            Self::Leaf { buffer_view, .. } => rows.push(buffer_view.render_row_at(ry)),
            Self::Split {
                frames,
                orientation: SplitOrientation::Vertical,
                ..
            } => {
                let mut cur_y = 0;
                for frame in frames {
                    if cur_y <= ry && ry < cur_y + frame.render_size().rows {
                        frame.push_render_rows_at(ry - cur_y, rows);
                        break;
                    }
                    cur_y += frame.render_size().rows;
                }
            }
        }
    }
}

pub(crate) struct RenderRows<'a> {
    frame: &'a Frame,
    ry: Range<usize>,
}

impl<'a> Iterator for RenderRows<'a> {
    type Item = Vec<(Segment, Ref<'a, Row>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let ry = self.ry.next()?;
        let mut rows = vec![];
        self.frame.push_render_rows_at(ry, &mut rows);
        Some(rows)
    }
}

fn split_size(render_size: Size, orientation: SplitOrientation) -> (Size, Size) {
    match orientation {
        SplitOrientation::Vertical => {
            let mut top_size = render_size;
            let mut bottom_size = render_size;
            bottom_size.rows /= 2;
            top_size.rows = render_size.rows - bottom_size.rows;
            (top_size, bottom_size)
        }
    }
}
