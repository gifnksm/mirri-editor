use crate::syntax::{Highlight, Syntax};
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

    fn invalidate_render(&mut self) {
        self.render_updated = false;
        self.highlight_updated = false;
    }

    pub(crate) fn invalidate_syntax(&mut self) {
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

    pub(crate) fn update_syntax(&mut self, syntax: &Syntax) {
        if self.highlight_updated {
            return;
        }
        self.update_render();

        self.highlight_updated = true;
        self.highlight.clear();

        let scs = syntax.singleline_comment_start;

        let mut prev_sep = true;
        let mut prev_hl = Highlight::Normal;
        let mut in_string = None;

        let kw1s = syntax
            .keyword1
            .iter()
            .copied()
            .zip(iter::repeat(Highlight::Keyword1));
        let kw2s = syntax
            .keyword2
            .iter()
            .copied()
            .zip(iter::repeat(Highlight::Keyword2));
        let keywords = kw1s.chain(kw2s);

        let mut chars = self.render.char_indices().fuse();
        while let Some((idx, ch)) = chars.next() {
            let mut highlight_len = ch.len_utf8();
            let highlight;
            let is_sep;
            #[allow(clippy::never_loop)]
            'outer: loop {
                if let Some(scs) = scs {
                    if in_string.is_none() && self.render[idx..].starts_with(scs) {
                        highlight_len += chars.by_ref().map(|(_, ch)| ch.len_utf8()).sum::<usize>();
                        highlight = Highlight::Comment;
                        is_sep = true;
                        break;
                    }
                }

                if syntax.string {
                    if let Some(delim) = in_string {
                        if ch == '\\' {
                            highlight_len += chars.next().map(|(_, ch)| ch.len_utf8()).unwrap_or(0);
                        } else if ch == delim {
                            in_string = None;
                        }
                        highlight = Highlight::String;
                        is_sep = true;
                        break;
                    }
                    if ch == '"' || ch == '\'' {
                        in_string = Some(ch);
                        highlight = Highlight::String;
                        is_sep = true;
                        break;
                    }
                }

                if syntax.number
                    && (ch.is_digit(10) && (prev_sep || prev_hl == Highlight::Number)
                        || (ch == '.' && prev_hl == Highlight::Number))
                {
                    highlight = Highlight::Number;
                    is_sep = false;
                    break;
                }

                if prev_sep {
                    let s = &self.render[idx..];
                    for (kw, hl_ty) in keywords.clone() {
                        if !s.starts_with(kw) {
                            continue;
                        }
                        let s = s.trim_start_matches(kw);
                        if !s.is_empty() && !s.starts_with(is_separator) {
                            continue;
                        }
                        highlight = hl_ty;
                        is_sep = false;
                        for _ in kw.chars().skip(1) {
                            highlight_len += chars.next().unwrap().1.len_utf8();
                        }
                        assert!(highlight_len == kw.len());
                        break 'outer;
                    }
                }

                highlight = Highlight::Normal;
                is_sep = is_separator(ch);
                break;
            }

            self.highlight
                .extend(iter::repeat(highlight).take(highlight_len));

            prev_hl = highlight;
            prev_sep = is_sep;
        }
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

fn is_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '\0' || ",.()+-/*=~%<>[];".contains(ch)
}
