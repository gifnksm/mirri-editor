use crate::{
    editor::Editor,
    input::{self, PromptCommand},
};

pub(crate) fn find(editor: &mut Editor) -> input::Result<()> {
    let saved_cx = editor.cx;
    let saved_cy = editor.cy;
    let saved_col_off = editor.col_off;
    let saved_row_off = editor.row_off;
    let _query = input::prompt_with_callback(
        editor,
        "Search: {} (ESC to cancel)",
        |editor, query, cmd| {
            use PromptCommand::*;

            match cmd {
                Input => {
                    'outer: for (y, row) in editor.rows.iter().enumerate() {
                        for (rx, _s) in row.render.match_indices(query.as_str()) {
                            editor.cy = y;
                            editor.cx = row.rx_to_cx(rx);
                            editor.row_off = editor.rows.len();
                            break 'outer;
                        }
                    }
                }
                Execute => {}
                Cancel => {
                    editor.cx = saved_cx;
                    editor.cy = saved_cy;
                    editor.col_off = saved_col_off;
                    editor.row_off = saved_row_off;
                }
            }
        },
    )?;
    Ok(())
}
