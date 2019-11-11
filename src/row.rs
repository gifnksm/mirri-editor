use crate::syntax::{Highlight, Syntax, SyntaxState};
use std::fmt::Write;

const TAB_STOP: usize = 8;

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) chars: String,
    render: String,
    render_updated: bool,
    syntax_state: SyntaxState,
}

impl Row {
    pub(crate) fn new(mut s: String) -> Self {
        s.truncate(s.trim_end_matches(&['\n', '\r'][..]).len());
        Row {
            chars: s,
            render: String::new(),
            render_updated: false,
            syntax_state: SyntaxState::new(),
        }
    }

    pub(crate) fn render(&self) -> &str {
        assert!(self.render_updated);
        &self.render
    }

    pub(crate) fn highlight(&self) -> &[Highlight] {
        self.syntax_state.highlight()
    }

    pub(crate) fn highlight_mut(&mut self) -> &mut Vec<Highlight> {
        self.syntax_state.highlight_mut()
    }

    fn invalidate_render(&mut self) {
        self.render_updated = false;
        self.syntax_state.invalidate();
    }

    pub(crate) fn invalidate_syntax(&mut self) {
        self.syntax_state.invalidate();
    }

    pub(crate) fn update_render(&mut self) {
        if self.render_updated {
            return;
        }

        self.render_updated = true;
        self.render.clear();

        let mut is_first = true;
        for s in self.chars.split('\t') {
            if !is_first {
                let width = TAB_STOP - self.render.len() % TAB_STOP;
                let _ = write!(&mut self.render, "{:w$}", "", w = width);
            } else {
                is_first = false;
            }
            self.render.push_str(s);
        }
    }

    pub(crate) fn update_syntax(
        &mut self,
        syntax: &Syntax,
        prev_row: Option<&mut Self>,
        next_row: Option<&mut Self>,
    ) {
        self.update_render();
        self.syntax_state.update(
            &self.render,
            syntax,
            prev_row.map(|row| &mut row.syntax_state),
            next_row.map(|row| &mut row.syntax_state),
        );
    }

    pub(crate) fn insert_char(&mut self, at: usize, ch: char) {
        self.chars.insert(at, ch);
        self.invalidate_render();
    }

    pub(crate) fn delete_char(&mut self, at: usize) {
        self.chars.remove(at);
        self.invalidate_render();
    }

    pub(crate) fn append_str(&mut self, s: &str) {
        self.chars.push_str(s.as_ref());
        self.invalidate_render();
    }

    pub(crate) fn split(&mut self, at: usize) -> String {
        let out = self.chars.split_off(at);
        if !out.is_empty() {
            self.invalidate_render();
        }
        out
    }

    pub(crate) fn cx_to_rx(&self, cx: usize) -> usize {
        let mut rx = 0;
        for ch in self.chars[..cx].bytes() {
            if ch == b'\t' {
                rx += TAB_STOP - rx % TAB_STOP;
            } else {
                rx += 1;
            }
        }
        rx
    }

    pub(crate) fn rx_to_cx(&self, rx: usize) -> usize {
        let mut cur_rx = 0;
        for (cx, ch) in self.chars.bytes().enumerate() {
            if ch == b'\t' {
                cur_rx += TAB_STOP - rx % TAB_STOP;
            } else {
                cur_rx += 1;
            }
            if cur_rx > rx {
                return cx;
            }
        }
        self.chars.len()
    }
}
