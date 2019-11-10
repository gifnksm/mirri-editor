use crate::{
    editor::Editor,
    input::{self, PromptCommand},
    syntax::Highlight,
};
use std::mem;

pub(crate) fn find(editor: &mut Editor) -> input::Result<()> {
    let saved_cx = editor.cx;
    let saved_cy = editor.cy;
    let saved_col_off = editor.col_off;
    let saved_row_off = editor.row_off;

    let mut saved_hl: Option<(usize, Vec<Highlight>)> = None;

    let mut last_match = None;
    let mut is_forward = true;

    let _query = input::prompt_with_callback(
        editor,
        "Search: {} (Use ESC/Arrow/Enter)",
        |editor, query, cmd| {
            use PromptCommand::*;

            if let Some((idx, hl)) = saved_hl.take() {
                let _ = mem::replace(editor.rows[idx].highlight_mut(), hl);
            }

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

            let (mut y, mut sx, mut ex) = last_match.unwrap_or((editor.cy, editor.rx, editor.rx));
            for _ in 0..editor.rows.len() {
                let row = &mut editor.rows[y];
                row.update_render();
                row.update_syntax();

                let (idx_off, res) = if is_forward {
                    (ex, row.render()[ex..].match_indices(query.as_str()).next())
                } else {
                    (0, row.render()[..sx].rmatch_indices(query.as_str()).next())
                };

                if let Some((dx, s)) = res {
                    let rx = idx_off + dx;
                    let s_len = s.len();
                    last_match = Some((y, rx, rx + s.len()));
                    editor.cy = y;
                    editor.cx = row.rx_to_cx(rx);
                    editor.row_off = row.render().len();
                    saved_hl = Some((y, row.highlight().into()));
                    for hl in &mut row.highlight_mut()[rx..rx + s_len] {
                        *hl = Highlight::Match
                    }
                    break;
                }

                if is_forward {
                    y = (y + 1) % editor.rows.len();
                } else if y == 0 {
                    y = editor.rows.len() - 1;
                } else {
                    y -= 1;
                }

                let row = &mut editor.rows[y];
                row.update_render();
                sx = row.render().len();
                ex = 0;
            }
        },
    )?;
    Ok(())
}
