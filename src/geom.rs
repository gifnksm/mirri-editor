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
