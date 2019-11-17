use crate::{
    decode::Decoder,
    editor::Editor,
    input::{self, PromptCommand},
    terminal::RawTerminal,
};

pub(crate) fn find(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
) -> input::Result<()> {
    let mut find = editor.find_start();
    let _query = input::prompt_with_callback(
        term,
        decoder,
        editor,
        "Search: {} (Use ESC/Arrow/Enter)",
        |editor, query, cmd| {
            use PromptCommand::*;
            match cmd {
                Input => find.input(editor, query),
                SearchBackward => find.search_backward(editor, query),
                SearchForward => find.search_forward(editor, query),
                Execute => find.execute(editor, query),
                Cancel => find.cancel(editor, query),
            }
        },
    )?;
    Ok(())
}
