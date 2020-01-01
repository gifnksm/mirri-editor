use crate::{
    decode::Decoder,
    editor::Editor,
    keypress::{self, PromptCommand},
    terminal::RawTerminal,
};

pub(crate) fn find(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
) -> keypress::Result<()> {
    let mut find = if let Some(find) = editor.find_start() {
        find
    } else {
        return Ok(());
    };

    let _query = keypress::prompt_with_callback(
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
