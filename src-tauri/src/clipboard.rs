//! Minimal clipboard access for paste mode.
//!
//! The web AI still runs in the user's browser — we never touch it. What we do
//! automate is the two Ctrl+C / Ctrl+V round-trips: the app puts the prompt on
//! the clipboard, and watches the clipboard so the reply can be picked up the
//! moment the user copies it.

use crate::error::CmdResult;

fn with_clipboard<T>(mut f: impl FnMut(&mut arboard::Clipboard) -> Result<T, arboard::Error>) -> CmdResult<T> {
    // Windows can refuse the clipboard for a few milliseconds while another
    // process (browser, IME) holds it; one retry covers that.
    let mut last = String::from("剪贴板不可用");
    for attempt in 0..3 {
        match arboard::Clipboard::new() {
            Ok(mut cb) => match f(&mut cb) {
                Ok(v) => return Ok(v),
                Err(e) => last = e.to_string(),
            },
            Err(e) => last = e.to_string(),
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }
    Err(last)
}

#[tauri::command(rename_all = "snake_case")]
pub fn clipboard_read_text() -> CmdResult<String> {
    with_clipboard(|cb| cb.get_text())
}

#[tauri::command(rename_all = "snake_case")]
pub fn clipboard_write_text(text: String) -> CmdResult<()> {
    with_clipboard(|cb| cb.set_text(text.clone()))
}

/// Whether the clipboard currently holds something we could take back.
/// Returns an empty string instead of an error when there is no text at all
/// (an image, an empty clipboard, or a locked clipboard).
#[tauri::command(rename_all = "snake_case")]
pub fn clipboard_peek() -> CmdResult<String> {
    Ok(with_clipboard(|cb| cb.get_text()).unwrap_or_default())
}
