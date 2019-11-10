use crate::syntax::{Syntax, SyntaxFlag};

pub(crate) const HLDB: &[Syntax] = &[Syntax {
    filetype: "c",
    filematch: &[".c", ".h", ".cpp"],
    flags: SyntaxFlag::NUMBERS,
}];
