use crate::{
    editor::Editor,
    input::{self, PromptCommand},
};

pub(crate) fn find(editor: &mut Editor) -> input::Result<()> {
    let saved_cx = editor.cx;
    let saved_cy = editor.cy;
    let saved_col_off = editor.col_off;
    let saved_row_off = editor.row_off;

    let mut last_match = None;
    let mut is_forward = true;

    let _query = input::prompt_with_callback(
        editor,
        "Search: {} (Use ESC/Arrow/Enter)",
        |editor, query, cmd| {
            use PromptCommand::*;

            dbg!(cmd, &query);
            match cmd {
                Input => {
                    last_match = None;
                }
                FindPrev => is_forward = false,
                FindNext => is_forward = true,
                Execute => {
                    return;
                }
                Cancel => {
                    editor.cx = saved_cx;
                    editor.cy = saved_cy;
                    editor.col_off = saved_col_off;
                    editor.row_off = saved_row_off;
                    return;
                }
            }

            let (mut y, mut sx) = last_match.unwrap_or((editor.cy, editor.rx));
            for _ in 0..editor.rows.len() {
                let row = &editor.rows[y];
                if let Some((dx, s)) = row.render[sx..].match_indices(query.as_str()).next() {
                    let rx = sx + dx;
                    last_match = Some((y, rx + s.len()));
                    editor.cy = y;
                    editor.cx = row.rx_to_cx(rx);
                    editor.row_off = editor.rows.len();
                    break;
                }
                sx = 0;
                if is_forward {
                    y = (y + 1) % editor.rows.len();
                } else if y == 0 {
                    y = editor.rows.len() - 1;
                } else {
                    y -= 1;
                }
            }
        },
    )?;
    Ok(())
}
