use crate::{
    geom::Segment,
    render::{RenderIndicesWithin, RenderItem, RenderStrExt},
    syntax::{Highlight, Syntax, SyntaxState},
};
use std::usize;

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

    pub(crate) fn render(&self) -> RenderIndicesWithin {
        self.chars.render_indices_within(0, self.render_segment)
    }

    pub(crate) fn render_with_highlight(&self) -> RenderWithHighlight {
        RenderWithHighlight {
            render: self.render(),
            row: self,
        }
    }

    pub(crate) fn get_rx_from_cx(&self, cx: usize) -> usize {
        self.chars[..cx].render_width(0)
    }

    pub(crate) fn get_cx_from_rx(&self, rx: usize) -> usize {
        self.chars.cx_from_rx(0, rx)
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
pub(crate) struct RenderWithHighlight<'a> {
    render: RenderIndicesWithin<'a>,
    row: &'a Row,
}

impl<'a> Iterator for RenderWithHighlight<'a> {
    type Item = (Highlight, RenderItem);

    fn next(&mut self) -> Option<Self::Item> {
        let (idx, item) = self.render.next()?;
        let hl = self.row.syntax_state.highlight_at(idx);
        Some((hl, item))
    }
}
