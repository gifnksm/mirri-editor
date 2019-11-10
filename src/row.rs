use crate::syntax::Highlight;
use std::{fmt::Write, iter};

const TAB_STOP: usize = 8;

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) chars: String,
    render: String,
    render_updated: bool,
    highlight: Vec<Highlight>,
    highlight_updated: bool,
}

impl Row {
    pub(crate) fn new(mut s: String) -> Self {
        s.truncate(s.trim_end_matches(&['\n', '\r'][..]).len());
        Row {
            chars: s,
            render: String::new(),
            render_updated: false,
            highlight: vec![],
            highlight_updated: false,
        }
    }

    pub(crate) fn render(&self) -> &str {
        assert!(self.render_updated);
        &self.render
    }

    pub(crate) fn highlight(&self) -> &[Highlight] {
        &self.highlight
    }

    pub(crate) fn highlight_mut(&mut self) -> &mut Vec<Highlight> {
        &mut self.highlight
    }

    fn invalidate(&mut self) {
        self.render_updated = false;
        self.highlight_updated = false;
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

    pub(crate) fn update_syntax(&mut self) {
        if self.highlight_updated {
            return;
        }
        self.update_render();

        self.highlight_updated = true;
        self.highlight.clear();

        let mut prev_sep = true;
        let mut prev_hl = Highlight::Normal;

        for ch in self.render.chars() {
            let (highlight, is_sep) = if ch.is_digit(10)
                && (prev_sep || prev_hl == Highlight::Number)
                || (ch == '.' && prev_hl == Highlight::Number)
            {
                (Highlight::Number, false)
            } else {
                (Highlight::Normal, is_separator(ch))
            };

            self.highlight
                .extend(iter::repeat(highlight).take(ch.len_utf8()));

            prev_hl = highlight;
            prev_sep = is_sep;
        }
    }

    pub(crate) fn insert_char(&mut self, at: usize, ch: char) {
        self.chars.insert(at, ch);
        self.invalidate();
    }

    pub(crate) fn delete_char(&mut self, at: usize) {
        self.chars.remove(at);
        self.invalidate();
    }

    pub(crate) fn append_str(&mut self, s: &str) {
        self.chars.push_str(s.as_ref());
        self.invalidate();
    }

    pub(crate) fn split(&mut self, at: usize) -> String {
        let out = self.chars.split_off(at);
        if !out.is_empty() {
            self.invalidate();
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

fn is_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '\0' || ",.()+-/*=~%<>[];".contains(ch)
}
