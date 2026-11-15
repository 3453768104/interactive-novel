//! 原生文件对话框。
//!
//! 用 rfd 而不是 tauri-plugin-dialog：后者的 `blocking_*` 明确禁止主线程调用，
//! 早期就是它把启动流程卡成黑屏。rfd 在独立线程上开系统对话框，不会碰 webview。

use crate::error::CmdResult;
use tauri::async_runtime::spawn_blocking;

fn with_filters(mut dlg: rfd::FileDialog) -> rfd::FileDialog {
    dlg = dlg.add_filter("Markdown", &["md", "markdown"]);
    dlg = dlg.add_filter("小说", &["epub", "docx"]);
    dlg = dlg.add_filter("所有文件", &["*"]);
    dlg
}

#[tauri::command]
pub async fn pick_file(title: Option<String>) -> CmdResult<Option<String>> {
    let t = title.unwrap_or_else(|| "选择文件".into());
    let picked = spawn_blocking(move || with_filters(rfd::FileDialog::new().set_title(t)).pick_file())
        .await
        .map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string_lossy().into()))
}

#[tauri::command]
pub async fn pick_save_path(
    default_name: Option<String>,
    title: Option<String>,
) -> CmdResult<Option<String>> {
    let t = title.unwrap_or_else(|| "保存为".into());
    let name = default_name.unwrap_or_else(|| "novel.md".into());
    let picked = spawn_blocking(move || {
        with_filters(rfd::FileDialog::new().set_title(t).set_file_name(name)).save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string_lossy().into()))
}

#[tauri::command]
pub async fn pick_folder(title: Option<String>) -> CmdResult<Option<String>> {
    let t = title.unwrap_or_else(|| "选择文件夹".into());
    let picked = spawn_blocking(move || rfd::FileDialog::new().set_title(t).pick_folder())
        .await
        .map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string_lossy().into()))
}
