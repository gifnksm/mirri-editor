use crate::syntax::Syntax;

pub(crate) const HLDB: &[Syntax] = &[Syntax {
    filetype: "c",
    filematch: &[".c", ".h", ".cpp"],
    number: true,
    string: true,
}];
