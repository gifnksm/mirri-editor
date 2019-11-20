use crate::geom::Segment;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    ops::Range,
    str::CharIndices,
};
use unicode_width::UnicodeWidthChar;

const TAB_STOP: usize = 8;

pub(crate) trait RenderStrExt {
    fn render_width(&self, start_col: usize) -> usize;
    fn cx_from_rx(&self, start_col: usize, rx: usize) -> usize;

    fn render_within(&self, start_col: usize, render_segment: Segment) -> RenderWithin;
    fn render_indices_within(
        &self,
        start_col: usize,
        render_segment: Segment,
    ) -> RenderIndicesWithin;
}

impl RenderStrExt for str {
    fn render_width(&self, start_col: usize) -> usize {
        let mut cur_col = start_col;
        for ch in self.chars() {
            let item = RenderItem::build(ch, cur_col);
            cur_col += item.width();
        }
        cur_col
    }
    fn cx_from_rx(&self, start_col: usize, rx: usize) -> usize {
        let mut cur_col = start_col;
        for (idx, ch) in self.char_indices() {
            if rx <= cur_col {
                return idx;
            }
            let item = RenderItem::build(ch, cur_col);
            cur_col += item.width();
            if cur_col > rx {
                return idx;
            }
        }
        self.len()
    }

    fn render_within(&self, start_col: usize, render_segment: Segment) -> RenderWithin {
        RenderWithin {
            inner: self.render_indices_within(start_col, render_segment),
        }
    }
    fn render_indices_within(
        &self,
        start_col: usize,
        render_segment: Segment,
    ) -> RenderIndicesWithin {
        RenderIndicesWithin {
            cur_col: start_col,
            render_segment,
            char_indices: self.char_indices(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderWithin<'a> {
    inner: RenderIndicesWithin<'a>,
}

impl<'a> Iterator for RenderWithin<'a> {
    type Item = RenderItem;
    fn next(&mut self) -> Option<Self::Item> {
        let (_idx, item) = self.inner.next()?;
        Some(item)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderIndicesWithin<'a> {
    cur_col: usize,
    render_segment: Segment,
    char_indices: CharIndices<'a>,
}

impl<'a> Iterator for RenderIndicesWithin<'a> {
    type Item = (usize, RenderItem);

    fn next(&mut self) -> Option<Self::Item> {
        let Range {
            start: scr_s,
            end: scr_e,
        } = self.render_segment.range();

        while let Some((idx, ch)) = self.char_indices.next() {
            let col_s = self.cur_col;
            if col_s >= scr_e {
                break;
            }

            let item = RenderItem::build(ch, col_s);
            self.cur_col += item.width();
            let col_e = self.cur_col;
            if col_e <= scr_s {
                continue;
            }
            if col_e > scr_e {
                break;
            }

            if col_s < scr_s {
                let width = col_e - scr_s;
                return Some((idx, RenderItem::padding(width)));
            }
            return Some((idx, item));
        }
        None
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderItem {
    width: usize,
    kind: RenderItemKind,
}

impl RenderItem {
    pub(crate) fn padding(width: usize) -> Self {
        RenderItem {
            width,
            kind: RenderItemKind::Padding,
        }
    }

    fn ascii_control(ch: char) -> Self {
        debug_assert!(ch.is_ascii_control());
        RenderItem {
            width: 2,
            kind: RenderItemKind::AsciiControl(ch as u8),
        }
    }

    fn unicode_control(ch: char) -> Self {
        debug_assert!(ch.is_control() && !ch.is_ascii_control());
        let byte = ch as u32;
        let width = if byte <= 0xffff {
            6
        } else {
            assert!(byte <= 0xfffff);
            7
        };
        RenderItem {
            width,
            kind: RenderItemKind::UnicodeControl(byte),
        }
    }

    pub(crate) fn char(ch: char) -> Self {
        assert!(!ch.is_control());
        let width = ch.width().unwrap();
        RenderItem {
            width,
            kind: RenderItemKind::Char(ch),
        }
    }

    pub(crate) fn build(ch: char, cur_col: usize) -> Self {
        if ch == '\t' {
            let width = TAB_STOP - cur_col % TAB_STOP;
            return Self::padding(width);
        }
        if ch.is_ascii_control() {
            return Self::ascii_control(ch);
        }
        if ch.is_control() {
            return Self::unicode_control(ch);
        }
        Self::char(ch)
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RenderItemKind {
    Padding,
    Char(char),
    AsciiControl(u8),
    UnicodeControl(u32),
}

impl Display for RenderItem {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use RenderItemKind::*;

        match self.kind {
            Padding => write!(f, "{:w$}", "", w = self.width),
            Char(ch) => write!(f, "{}", ch),
            AsciiControl(ch) => write!(f, "^{}", (ch as u8 ^ 0x40) as char),
            UnicodeControl(ch) => {
                let byte = ch as u32;
                if byte <= 0xffff {
                    write!(f, "U+{:04X}", byte)
                } else {
                    assert!(byte <= 0xfffff);
                    write!(f, "U+{:05X}", byte)
                }
            }
        }
    }
}
