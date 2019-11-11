use std::{ffi::OsStr, iter, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct Syntax<'a> {
    pub(crate) filetype: &'a str,
    pub(crate) filematch: &'a [&'a str],
    pub(crate) number: bool,
    pub(crate) string: bool,
    pub(crate) singleline_comment_start: &'a [&'a str],
    pub(crate) multiline_comment: &'a [(&'a str, &'a str)],
    pub(crate) string_literal: &'a [(&'a str, &'a str)],
    pub(crate) keyword1: &'a [&'a str],
    pub(crate) keyword2: &'a [&'a str],
}

const DEFAULT: Syntax = Syntax {
    filetype: "no ft",
    filematch: &[],
    number: false,
    string: false,
    singleline_comment_start: &[],
    multiline_comment: &[],
    string_literal: &[],
    keyword1: &[],
    keyword2: &[],
};

const HLDB: &[Syntax] = &[Syntax {
    filetype: "c",
    filematch: &[".c", ".h", ".cpp"],
    number: true,
    string: true,
    singleline_comment_start: &["//"],
    multiline_comment: &[("/*", "*/")],
    string_literal: &[("\"", "\""), ("'", "'")],
    keyword1: &[
        "switch", "if", "while", "for", "break", "continue", "return", "else", "struct", "union",
        "typedef", "static", "enum", "class", "case",
    ],
    keyword2: &[
        "int", "long", "double", "float", "char", "unsigned", "signed", "void",
    ],
}];

impl<'a> Syntax<'a> {
    pub(crate) fn keywords(&self) -> impl Iterator<Item = (&'a str, Highlight)> + Clone {
        let kw1s = self
            .keyword1
            .iter()
            .copied()
            .zip(iter::repeat(Highlight::Keyword1));
        let kw2s = self
            .keyword2
            .iter()
            .copied()
            .zip(iter::repeat(Highlight::Keyword2));
        kw1s.chain(kw2s)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Highlight {
    Normal,
    SingleLineComment,
    MultiLineComment,
    Keyword1,
    Keyword2,
    String,
    Number,
    Match,
}

impl Highlight {
    pub(crate) fn to_color(self) -> u32 {
        match self {
            Self::Normal => 37,
            Self::SingleLineComment | Self::MultiLineComment => 36,
            Self::Keyword1 => 33,
            Self::Keyword2 => 32,
            Self::String => 35,
            Self::Number => 31,
            Self::Match => 34,
        }
    }
}

pub(crate) fn select(filename: Option<impl AsRef<Path>>) -> &'static Syntax<'static> {
    select_from_hldb(filename).unwrap_or(&DEFAULT)
}

fn select_from_hldb(filename: Option<impl AsRef<Path>>) -> Option<&'static Syntax<'static>> {
    let filename = filename?;
    let filename = filename.as_ref();
    let name = filename.file_name();
    let ext = filename.extension();

    for syntax in HLDB {
        let is_match = syntax.filematch.iter().copied().any(|m| {
            let is_ext = m.starts_with('.');
            if is_ext {
                ext == Some(OsStr::new(m.trim_start_matches('.')))
            } else {
                name == Some(OsStr::new(m))
            }
        });
        if is_match {
            return Some(syntax);
        }
    }
    None
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Open {
    Comment(&'static str),
    String(&'static str),
}

#[derive(Debug, Clone)]
pub(crate) struct SyntaxState {
    updated: bool,
    open: Option<Open>,
    highlight: Vec<Highlight>,
}

impl SyntaxState {
    pub(crate) fn new() -> Self {
        SyntaxState {
            updated: false,
            open: None,
            highlight: vec![],
        }
    }

    pub(crate) fn highlight(&self) -> &[Highlight] {
        &self.highlight
    }

    pub(crate) fn highlight_mut(&mut self) -> &mut Vec<Highlight> {
        &mut self.highlight
    }

    pub(crate) fn invalidate(&mut self) {
        self.updated = false;
    }

    pub(crate) fn update(
        &mut self,
        render: &str,
        syntax: &'static Syntax,
        prev: Option<&mut SyntaxState>,
        next: Option<&mut SyntaxState>,
    ) {
        if self.updated {
            return;
        }

        self.updated = true;
        self.highlight.clear();

        let scs = syntax.singleline_comment_start;
        let mc = syntax.multiline_comment;
        let sl = syntax.string_literal;

        let mut prev_sep = true;
        let mut open = prev.and_then(|state| state.open);
        let keywords = syntax.keywords();

        let mut chars = render;
        while !chars.is_empty() {
            let highlight_len;
            let highlight;
            let is_sep;
            match open {
                Some(Open::String(sle)) => {
                    let mut found = None;
                    let mut escaped = None;
                    let sle_head = sle.chars().next().unwrap();
                    for (idx, m) in chars.match_indices(&[sle_head, '\\'][..]) {
                        if escaped == Some(idx) {
                            continue;
                        }
                        if m.starts_with('\\') {
                            escaped = Some(idx + '\\'.len_utf8());
                            continue;
                        }
                        if m.starts_with(sle) {
                            found = Some(idx + sle.len());
                            break;
                        }
                    }

                    highlight = Highlight::String;
                    if let Some(len) = found {
                        highlight_len = len;
                        open = None;
                    } else {
                        highlight_len = chars.len();
                    }
                    is_sep = true;
                }
                Some(Open::Comment(mce)) => {
                    highlight = Highlight::MultiLineComment;
                    if let Some((idx, _)) = chars.match_indices(mce).next() {
                        highlight_len = idx + mce.len();
                        open = None;
                    } else {
                        highlight_len = chars.len();
                    }
                    is_sep = true;
                }
                None =>
                {
                    #[allow(clippy::never_loop)]
                    'outer: loop {
                        for scs in scs {
                            if chars.starts_with(scs) {
                                highlight = Highlight::SingleLineComment;
                                highlight_len = chars.len();
                                is_sep = true;
                                break 'outer;
                            }
                        }

                        for (mcs, mce) in mc {
                            if chars.starts_with(mcs) {
                                open = Some(Open::Comment(mce));
                                highlight = Highlight::MultiLineComment;
                                highlight_len = mcs.len();
                                is_sep = true;
                                break 'outer;
                            }
                        }

                        for (sls, sle) in sl {
                            if chars.starts_with(sls) {
                                open = Some(Open::String(sle));
                                highlight = Highlight::String;
                                highlight_len = sls.len();
                                is_sep = true;
                                break 'outer;
                            }
                        }

                        if prev_sep {
                            if syntax.number {
                                let t = chars.trim_start_matches(|ch: char| ch.is_digit(10));
                                let match_len = chars.len() - t.len();
                                if match_len > 0 {
                                    let t = t.trim_start_matches(|ch: char| {
                                        ch.is_digit(10) || ch == '.'
                                    });
                                    let match_len = chars.len() - t.len();
                                    highlight = Highlight::Number;
                                    highlight_len = match_len;
                                    is_sep = true;
                                    break;
                                }
                            }

                            for (kw, hl_ty) in keywords.clone() {
                                let t = chars.trim_start_matches(kw);
                                let match_len = chars.len() - t.len();
                                if match_len == 0 {
                                    continue;
                                }
                                if !t.is_empty() && !t.starts_with(is_separator) {
                                    continue;
                                }
                                highlight = hl_ty;
                                highlight_len = match_len;
                                is_sep = false;
                                break 'outer;
                            }
                        }

                        let ch = chars.chars().next().unwrap();
                        highlight = Highlight::Normal;
                        highlight_len = ch.len_utf8();
                        is_sep = is_separator(ch);
                        break;
                    }
                }
            }

            self.highlight
                .extend(iter::repeat(highlight).take(highlight_len));
            chars = &chars[highlight_len..];
            prev_sep = is_sep;
        }

        let changed = self.open != open;
        self.open = open;
        if changed {
            if let Some(next) = next {
                next.invalidate();
            }
        }
    }
}

fn is_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '\0' || ",.()+-/*=~%<>[];".contains(ch)
}
