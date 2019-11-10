use crate::filetypes::HLDB;
use bitflags::bitflags;
use std::{ffi::OsStr, path::Path};

bitflags! {
    pub(crate) struct SyntaxFlag: u32 {
        const NUMBERS = 1 << 0;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Syntax<'a> {
    pub(crate) filetype: &'a str,
    pub(crate) filematch: &'a [&'a str],
    pub(crate) flags: SyntaxFlag,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Highlight {
    Normal,
    Number,
    Match,
}

impl Highlight {
    pub(crate) fn to_color(self) -> u32 {
        match self {
            Self::Normal => 37,
            Self::Number => 31,
            Self::Match => 34,
        }
    }
}

pub(crate) fn select(filename: Option<impl AsRef<Path>>) -> Option<&'static Syntax<'static>> {
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
