use std::{ffi::OsStr, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct Syntax<'a> {
    pub(crate) filetype: &'a str,
    pub(crate) filematch: &'a [&'a str],
    pub(crate) number: bool,
    pub(crate) string: bool,
    pub(crate) singleline_comment_start: Option<&'a str>,
    pub(crate) keyword1: &'a [&'a str],
    pub(crate) keyword2: &'a [&'a str],
}

const DEFAULT: Syntax = Syntax {
    filetype: "no ft",
    filematch: &[],
    number: false,
    string: false,
    singleline_comment_start: None,
    keyword1: &[],
    keyword2: &[],
};

const HLDB: &[Syntax] = &[Syntax {
    filetype: "c",
    filematch: &[".c", ".h", ".cpp"],
    number: true,
    string: true,
    singleline_comment_start: Some("//"),
    keyword1: &[
        "switch", "if", "while", "for", "break", "continue", "return", "else", "struct", "union",
        "typedef", "static", "enum", "class", "case",
    ],
    keyword2: &[
        "int", "long", "double", "float", "char", "unsigned", "signed", "void",
    ],
}];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Highlight {
    Normal,
    Comment,
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
            Self::Comment => 36,
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
