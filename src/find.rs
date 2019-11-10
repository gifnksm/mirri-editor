use crate::{editor::Editor, input};

pub(crate) fn find(editor: &mut Editor) -> input::Result<()> {
    let query = if let Some(query) = input::prompt(editor, "Search: {} (ESC to cancel)")? {
        query
    } else {
        return Ok(());
    };

    'outer: for (y, row) in editor.rows.iter().enumerate() {
        for (rx, _s) in row.render.match_indices(&query) {
            editor.cy = y;
            editor.cx = row.rx_to_cx(rx);
            editor.row_off = editor.rows.len();
            break 'outer;
        }
    }

    Ok(())
}
