use std::{ffi::OsStr, iter, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct Syntax<'a> {
    pub(crate) filetype: &'a str,
    pub(crate) filematch: &'a [&'a str],
    pub(crate) number: bool,
    pub(crate) string: bool,
    pub(crate) singleline_comment_start: Option<&'a str>,
    pub(crate) multiline_comment: Option<(&'a str, &'a str)>,
    pub(crate) keyword1: &'a [&'a str],
    pub(crate) keyword2: &'a [&'a str],
}

const DEFAULT: Syntax = Syntax {
    filetype: "no ft",
    filematch: &[],
    number: false,
    string: false,
    singleline_comment_start: None,
    multiline_comment: None,
    keyword1: &[],
    keyword2: &[],
};

const HLDB: &[Syntax] = &[Syntax {
    filetype: "c",
    filematch: &[".c", ".h", ".cpp"],
    number: true,
    string: true,
    singleline_comment_start: Some("//"),
    multiline_comment: Some(("/*", "*/")),
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

#[derive(Debug, Clone)]
pub(crate) struct SyntaxState {
    updated: bool,
    open_comment: bool,
    highlight: Vec<Highlight>,
}

impl SyntaxState {
    pub(crate) fn new() -> Self {
        SyntaxState {
            updated: false,
            open_comment: false,
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
        syntax: &Syntax,
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

        let mut prev_sep = true;
        let mut prev_hl = Highlight::Normal;
        let mut in_string = None;
        let mut in_ml_comment = prev.map(|state| state.open_comment).unwrap_or(false);
        let keywords = syntax.keywords();

        let mut chars = render.char_indices().fuse();
        while let Some((idx, ch)) = chars.next() {
            let mut highlight_len = ch.len_utf8();
            let highlight;
            let is_sep;
            #[allow(clippy::never_loop)]
            'outer: loop {
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

                if in_ml_comment {
                    let (mcs, mce) = mc.unwrap();
                    highlight = Highlight::MultiLineComment;
                    if render[idx..].starts_with(mce) {
                        in_ml_comment = false;
                        for _ in mcs.chars().skip(1) {
                            highlight_len += chars.next().unwrap().1.len_utf8();
                        }
                        assert!(highlight_len == mce.len());
                    }
                    is_sep = true;
                    break;
                }

                if let Some(scs) = scs {
                    if render[idx..].starts_with(scs) {
                        highlight_len += chars.by_ref().map(|(_, ch)| ch.len_utf8()).sum::<usize>();
                        highlight = Highlight::SingleLineComment;
                        is_sep = true;
                        break;
                    }
                }

                if let Some((mcs, mce)) = mc {
                    if render[idx..].starts_with(mcs) {
                        in_ml_comment = true;
                        highlight = Highlight::MultiLineComment;
                        for _ in mcs.chars().skip(1) {
                            highlight_len += chars.next().unwrap().1.len_utf8();
                        }
                        assert!(highlight_len == mce.len());
                        is_sep = true;
                        break;
                    }
                }

                if syntax.string && (ch == '"' || ch == '\'') {
                    in_string = Some(ch);
                    highlight = Highlight::String;
                    is_sep = true;
                    break;
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
                    let s = &render[idx..];
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

        let changed = self.open_comment != in_ml_comment;
        self.open_comment = in_ml_comment;
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
