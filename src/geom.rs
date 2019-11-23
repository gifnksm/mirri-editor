use std::ops::Range;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct Point {
    pub(crate) x: usize,
    pub(crate) y: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct Size {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Default)]
pub(crate) struct Rect {
    pub(crate) origin: Point,
    pub(crate) size: Size,
}

impl Rect {
    pub(crate) fn x_segment(self) -> Segment {
        Segment {
            origin: self.origin.x,
            size: self.size.cols,
        }
    }
    pub(crate) fn y_segment(self) -> Segment {
        Segment {
            origin: self.origin.y,
            size: self.size.rows,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Default)]
pub(crate) struct Segment {
    pub(crate) origin: usize,
    pub(crate) size: usize,
}

impl Segment {
    pub(crate) fn range(self) -> Range<usize> {
        self.origin..self.origin + self.size
    }
}
