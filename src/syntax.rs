use std::{ffi::OsStr, iter, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct Syntax<'a> {
    pub(crate) filetype: &'a str,
    pub(crate) filematch: &'a [&'a str],
    pub(crate) number: bool,
    pub(crate) single_line_comment: &'a [&'a str],
    pub(crate) multi_line_comment: &'a [(&'a str, &'a str)],
    pub(crate) string_literal: &'a [(&'a str, &'a str)],
    pub(crate) keyword1: &'a [&'a str],
    pub(crate) keyword2: &'a [&'a str],
}

const DEFAULT: Syntax = Syntax {
    filetype: "no ft",
    filematch: &[],
    number: false,
    single_line_comment: &[],
    multi_line_comment: &[],
    string_literal: &[],
    keyword1: &[],
    keyword2: &[],
};

const HLDB: &[Syntax] = &[
    Syntax {
        filetype: "c",
        filematch: &[".c", ".h", ".cpp"],
        number: true,
        single_line_comment: &["//"],
        multi_line_comment: &[("/*", "*/")],
        string_literal: &[("\"", "\""), ("'", "'")],
        keyword1: &[
            "switch", "if", "while", "for", "break", "continue", "return", "else", "struct",
            "union", "typedef", "static", "enum", "class", "case",
        ],
        keyword2: &[
            "int", "long", "double", "float", "char", "unsigned", "signed", "void",
        ],
    },
    Syntax {
        filetype: "rust",
        filematch: &[".rs"],
        number: true,
        single_line_comment: &["//"],
        multi_line_comment: &[("/*", "*/")],
        string_literal: &[("\"", "\""), ("'", "'")],
        keyword1: &[
            "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
            "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
            "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while",
        ],
        keyword2: &[
            "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "bool", "char",
            "f32", "f64",
        ],
    },
];

impl<'s> Syntax<'s> {
    pub(crate) fn select(filename: Option<impl AsRef<Path>>) -> &'static Syntax<'static> {
        Self::select_from_hldb(filename).unwrap_or(&DEFAULT)
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

    fn parse(
        &'s self,
        chars: &str,
        prev_sep: &mut bool,
        open: &mut Option<Open<'s>>,
    ) -> (Highlight, usize) {
        match open {
            Some(Open::String(sle)) => {
                let (len, new_open) = self.parse_string_literal_end(chars, sle);
                *prev_sep = true;
                *open = new_open;
                (Highlight::String, len)
            }
            Some(Open::Comment(mce)) => {
                let (len, new_open) = self.parse_multi_line_comment_end(chars, mce);
                *prev_sep = true;
                *open = new_open;
                (Highlight::MultiLineComment, len)
            }
            None => {
                if let Some(len) = self.parse_single_line_comment(chars) {
                    *prev_sep = true;
                    (Highlight::SingleLineComment, len)
                } else if let Some((len, mce)) = self.parse_multi_line_comment_start(chars) {
                    *prev_sep = true;
                    *open = Some(Open::Comment(mce));
                    (Highlight::MultiLineComment, len)
                } else if let Some((len, sle)) = self.parse_string_literal_start(chars) {
                    *prev_sep = true;
                    *open = Some(Open::String(sle));
                    (Highlight::String, len)
                } else if let Some(len) = self.parse_number(chars, *prev_sep) {
                    *prev_sep = false;
                    (Highlight::Number, len)
                } else if let Some(len) = self.parse_keyword1(chars, *prev_sep) {
                    *prev_sep = false;
                    (Highlight::Keyword1, len)
                } else if let Some(len) = self.parse_keyword2(chars, *prev_sep) {
                    *prev_sep = false;
                    (Highlight::Keyword2, len)
                } else {
                    let ch = chars.chars().next().unwrap();
                    *prev_sep = is_separator(ch);
                    (Highlight::Normal, ch.len_utf8())
                }
            }
        }
    }

    fn parse_single_line_comment(&self, chars: &str) -> Option<usize> {
        for scs in self.single_line_comment {
            if chars.starts_with(scs) {
                return Some(chars.len());
            }
        }
        None
    }

    fn parse_multi_line_comment_start(&self, chars: &str) -> Option<(usize, &str)> {
        for (mcs, mce) in self.multi_line_comment {
            if chars.starts_with(mcs) {
                return Some((mcs.len(), *mce));
            }
        }
        None
    }

    fn parse_multi_line_comment_end<'a>(
        &self,
        chars: &str,
        mce: &'a str,
    ) -> (usize, Option<Open<'a>>) {
        if let Some((idx, _)) = chars.match_indices(mce).next() {
            (idx + mce.len(), None)
        } else {
            (chars.len(), Some(Open::Comment(mce)))
        }
    }

    fn parse_string_literal_start(&self, chars: &str) -> Option<(usize, &str)> {
        for (sls, sle) in self.string_literal {
            if chars.starts_with(sls) {
                return Some((sls.len(), *sle));
            }
        }
        None
    }

    fn parse_string_literal_end<'a>(&self, chars: &str, sle: &'a str) -> (usize, Option<Open<'a>>) {
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
                return (idx + sle.len(), None);
            }
        }
        (chars.len(), Some(Open::String(sle)))
    }

    fn parse_number(&self, chars: &str, prev_sep: bool) -> Option<usize> {
        if !prev_sep || !self.number {
            return None;
        }

        let t = chars.trim_start_matches(|ch: char| ch.is_digit(10));
        if chars.len() != t.len() {
            let t = t.trim_start_matches(|ch: char| ch.is_digit(10) || ch == '.');
            Some(chars.len() - t.len())
        } else {
            None
        }
    }

    fn parse_keyword_common(&self, chars: &str, prev_sep: bool, kws: &[&str]) -> Option<usize> {
        if !prev_sep {
            return None;
        }
        for kw in kws {
            if !chars.starts_with(kw) {
                continue;
            }
            let t = &chars[kw.len()..];
            if t.is_empty() || t.starts_with(is_separator) {
                return Some(kw.len());
            }
        }
        None
    }

    fn parse_keyword1(&self, chars: &str, prev_sep: bool) -> Option<usize> {
        self.parse_keyword_common(chars, prev_sep, self.keyword1)
    }
    fn parse_keyword2(&self, chars: &str, prev_sep: bool) -> Option<usize> {
        self.parse_keyword_common(chars, prev_sep, self.keyword2)
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Open<'a> {
    Comment(&'a str),
    String(&'a str),
}

#[derive(Debug, Clone)]
pub(crate) struct SyntaxState {
    updated: bool,
    open: Option<Open<'static>>,
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
        assert!(self.updated);
        &self.highlight
    }

    pub(crate) fn highlight_mut(&mut self) -> &mut Vec<Highlight> {
        assert!(self.updated);
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

        let mut prev_sep = true;
        let mut open = prev.and_then(|state| state.open);

        let mut chars = render;
        while !chars.is_empty() {
            let (highlight, len) = syntax.parse(chars, &mut prev_sep, &mut open);
            self.highlight.extend(iter::repeat(highlight).take(len));
            chars = &chars[len..];
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
