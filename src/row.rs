use crate::{
    geom::Segment,
    syntax::{Highlight, Syntax, SyntaxState},
};
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    ops::Range,
    str::CharIndices,
    usize,
};
use unicode_width::UnicodeWidthChar;

const TAB_STOP: usize = 8;

#[derive(Debug, Clone)]
pub(crate) struct Row {
    chars: String,
    render_segment: Segment,
    syntax_state: SyntaxState,
}

impl Row {
    pub(crate) fn new(mut s: String, render_segment: Segment) -> Self {
        s.truncate(s.trim_end_matches(&['\n', '\r'][..]).len());
        Row {
            chars: s,
            syntax_state: SyntaxState::new(),
            render_segment,
        }
    }

    pub(crate) fn chars(&self) -> &str {
        &self.chars
    }

    pub(crate) fn set_render_size(&mut self, render_segment: Segment) {
        self.render_segment = render_segment;
    }

    pub(crate) fn syntax_mut(&mut self) -> &mut SyntaxState {
        &mut self.syntax_state
    }

    pub(crate) fn invalidate_syntax(&mut self) {
        self.syntax_state.invalidate();
    }

    pub(crate) fn update_highlight(
        &mut self,
        syntax: &'static Syntax,
        prev_row: Option<&mut Self>,
        next_row: Option<&mut Self>,
    ) {
        self.syntax_state.update(
            &self.chars,
            syntax,
            prev_row.map(|row| &mut row.syntax_state),
            next_row.map(|row| &mut row.syntax_state),
        );
    }

    pub(crate) fn render(&self) -> Render {
        Render {
            cur_col: 0,
            render_segment: self.render_segment,
            chars: self.chars.char_indices(),
        }
    }

    pub(crate) fn render_with_highlight(&self) -> RenderWithHighlight {
        RenderWithHighlight {
            render: self.render(),
            row: self,
        }
    }

    pub(crate) fn get_rx_from_cx(&self, cx: usize) -> usize {
        let mut cur_col = 0;
        for (idx, ch) in self.chars[..cx].char_indices() {
            let item = RenderItem::build(idx, ch, cur_col);
            cur_col += item.width;
        }
        cur_col
    }

    pub(crate) fn get_cx_from_rx(&self, rx: usize) -> usize {
        let mut cur_col = 0;
        for (idx, ch) in self.chars.char_indices() {
            if rx == cur_col {
                return idx;
            }
            let item = RenderItem::build(idx, ch, cur_col);
            if cur_col + item.width > rx {
                return item.idx;
            }
            cur_col += item.width;
        }
        self.chars.len()
    }

    pub(crate) fn insert_char(&mut self, at: usize, ch: char) {
        self.chars.insert(at, ch);
        self.invalidate_syntax();
    }

    pub(crate) fn delete_char(&mut self, at: usize) {
        self.chars.remove(at);
        self.invalidate_syntax();
    }

    pub(crate) fn append_str(&mut self, s: &str) {
        self.chars.push_str(s.as_ref());
        self.invalidate_syntax();
    }

    pub(crate) fn split(&mut self, at: usize) -> String {
        let out = self.chars.split_off(at);
        if !out.is_empty() {
            self.invalidate_syntax();
        }
        out
    }
}

#[derive(Debug)]
pub(crate) struct RenderItem {
    idx: usize,
    width: usize,
    kind: RenderItemKind,
}

impl RenderItem {
    fn padding(idx: usize, width: usize) -> Self {
        RenderItem {
            idx,
            width,
            kind: RenderItemKind::Padding,
        }
    }

    fn ascii_control(idx: usize, ch: char) -> Self {
        debug_assert!(ch.is_ascii_control());
        RenderItem {
            idx,
            width: 2,
            kind: RenderItemKind::AsciiControl(ch as u8),
        }
    }

    fn unicode_control(idx: usize, ch: char) -> Self {
        debug_assert!(ch.is_control() && !ch.is_ascii_control());
        let byte = ch as u32;
        let width = if byte <= 0xffff {
            6
        } else {
            assert!(byte <= 0xfffff);
            7
        };
        RenderItem {
            idx,
            width,
            kind: RenderItemKind::UnicodeControl(byte),
        }
    }

    fn char(idx: usize, ch: char) -> Self {
        assert!(!ch.is_control());
        let width = ch.width().unwrap();
        RenderItem {
            idx,
            width,
            kind: RenderItemKind::Char(ch),
        }
    }

    fn build(idx: usize, ch: char, cur_col: usize) -> Self {
        if ch == '\t' {
            let width = TAB_STOP - cur_col % TAB_STOP;
            return Self::padding(idx, width);
        }
        if ch.is_ascii_control() {
            return Self::ascii_control(idx, ch);
        }
        if ch.is_control() {
            return Self::unicode_control(idx, ch);
        }
        Self::char(idx, ch)
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub(crate) struct Render<'a> {
    cur_col: usize,
    render_segment: Segment,
    chars: CharIndices<'a>,
}

impl<'a> Iterator for Render<'a> {
    type Item = RenderItem;

    fn next(&mut self) -> Option<Self::Item> {
        let Range {
            start: scr_s,
            end: scr_e,
        } = self.render_segment.range();

        while let Some((idx, ch)) = self.chars.next() {
            let col_s = self.cur_col;
            if col_s >= scr_e {
                break;
            }

            let item = RenderItem::build(idx, ch, col_s);
            self.cur_col += item.width;
            let col_e = self.cur_col;
            if col_e <= scr_s {
                continue;
            }

            if col_s < scr_s {
                let width = col_e - scr_s;
                return Some(RenderItem::padding(idx, width));
            }
            if col_e > scr_e {
                let width = scr_e - col_s;
                return Some(RenderItem::padding(idx, width));
            }
            return Some(item);
        }
        None
    }
}

#[derive(Debug)]
pub(crate) struct RenderWithHighlight<'a> {
    render: Render<'a>,
    row: &'a Row,
}

impl<'a> Iterator for RenderWithHighlight<'a> {
    type Item = (Highlight, RenderItem);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.render.next()?;
        let hl = self.row.syntax_state.highlight_at(item.idx);
        Some((hl, item))
    }
}
