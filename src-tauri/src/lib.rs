mod chapters;
mod clipboard;
mod commands;
pub mod commands_ai;
mod commands_config;
mod dialogs;
mod diagnostics;
mod commands_lore;
mod commands_v2;
mod commands_v3;
mod config_migrate;
mod context;
// Public so `examples/seed_lore.rs` can seed a world through the exact same
// code path the app uses (chunking, sha, mirror files).
pub mod db;
pub mod defaults;
pub mod domain;
pub mod engine;
pub mod export;
pub mod error;
pub mod importer;
pub mod lore;
pub mod retrieval;
pub mod search;
pub mod state;
mod mirror;

use std::path::{Path, PathBuf};
use tauri::Manager;

use state::AppState;

// Windows-native message box. Tauri's dialog plugin uses a sync_channel recv()
// and explicitly forbids main-thread usage; setup() runs on the main thread, so
// using it there deadlocks before the webview ever loads (black window).
// MessageBoxW is synchronous, owns its own message loop and needs no webview.
#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        h_wnd: *mut std::ffi::c_void,
        lp_text: *const u16,
        lp_caption: *const u16,
        u_type: u32,
    ) -> i32;
}

fn ask_yes_no(title: &str, message: &str) -> bool {
    #[cfg(windows)]
    {
        const MB_YESNO: u32 = 0x0000_0004;
        const MB_ICONQUESTION: u32 = 0x0000_0020;
        const MB_TOPMOST: u32 = 0x0004_0000;
        const IDYES: i32 = 6;
        let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let caption: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_YESNO | MB_ICONQUESTION | MB_TOPMOST,
            )
        };
        return result == IDYES;
    }
    #[cfg(not(windows))]
    {
        let _ = (title, message);
        false
    }
}

/// Portable layout: everything lives in `<exe dir>/data/`.
/// NOVEL_DATA_DIR is an escape hatch for development and unusual deployments.
fn default_data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("NOVEL_DATA_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let parent = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| exe.clone());
    parent.join("data")
}

/// The v1 location, used only to offer a one-time migration. Never a default.
fn legacy_data_dir() -> Option<PathBuf> {
    let base = std::env::var("APPDATA").ok().map(PathBuf::from)?;
    let legacy = base.join("interactive-novel");
    if legacy.join("novel.db").exists() {
        Some(legacy)
    } else {
        None
    }
}

fn ensure_writable(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| {
        format!(
            "无法创建数据目录 {}：{e}。\n请勿把程序放在 Program Files 等只读位置；也可设置环境变量 NOVEL_DATA_DIR 指向可写目录。",
            dir.display()
        )
    })?;
    let probe = dir.join(".write_probe");
    std::fs::write(&probe, b"ok").map_err(|e| {
        format!(
            "数据目录 {} 不可写：{e}。请移动程序到可写位置，或设置 NOVEL_DATA_DIR。",
            dir.display()
        )
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Must run BEFORE Database::open: once the target DB is opened in WAL mode,
/// swapping its files underneath would be unsafe.
fn offer_legacy_migration(target: &Path, legacy: &Path) -> Result<Option<String>, String> {
    // Already migrated, or the user explicitly declined -> never ask again.
    if target.join("novel.db").exists()
        || target.join(".migrated_from").exists()
        || target.join(".migration_skipped").exists()
    {
        return Ok(None);
    }
    let worlds = std::fs::read_dir(legacy.join("worlds"))
        .ok()
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count()
        })
        .unwrap_or(0);

    let message = format!(
        "检测到旧版本数据（{} 个世界观）：\n{}\n\n是否复制到便携数据目录？\n\n【是】复制旧数据（含数据库与 WAL，最近生成的正文不会丢）\n【否】跳过，从零开始\n\n原目录不会被删除或改动。",
        worlds,
        legacy.display()
    );
    let answer = ask_yes_no("互动小说 · 迁移旧数据", &message);

    if !answer {
        let _ = std::fs::write(
            target.join(".migration_skipped"),
            legacy.to_string_lossy().as_bytes(),
        );
        return Ok(None);
    }

    // Cold copy of the WAL trio. Copying only novel.db would lose uncheckpointed
    // passages that are still in novel.db-wal.
    for name in ["novel.db", "novel.db-wal", "novel.db-shm"] {
        let from = legacy.join(name);
        if from.exists() {
            std::fs::copy(&from, target.join(name))
                .map_err(|e| format!("复制 {name} 失败：{e}"))?;
        }
    }
    for name in ["config", "worlds", "backups"] {
        let from = legacy.join(name);
        if from.exists() {
            config_migrate::copy_dir_all(&from, &target.join(name))
                .map_err(|e| format!("复制 {name}/ 失败：{e}"))?;
        }
    }
    let _ = std::fs::write(
        target.join(".migrated_from"),
        legacy.to_string_lossy().as_bytes(),
    );
    Ok(Some(legacy.to_string_lossy().into_owned()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = default_data_dir();
            ensure_writable(&data_dir)?;

            let mut migrated_from = None;
            if let Some(legacy) = legacy_data_dir() {
                migrated_from = offer_legacy_migration(&data_dir, &legacy)?;
            }

            let app_state = AppState::new(data_dir.clone(), migrated_from)
                .map_err(|e| format!("初始化数据目录失败: {e}"))?;

            // Push v2 factory defaults into an existing config/ without ever
            // overwriting a file the user edited (engines.yaml may hold a key).
            let needs_merge = config_migrate::ensure(&app_state).unwrap_or_else(|e| {
                eprintln!("[config migrate] {e}");
                Vec::new()
            });
            *app_state.needs_merge.lock().unwrap_or_else(|p| p.into_inner()) = needs_merge;

            // One-shot repair of historical glued-together options.
            if let Err(e) = config_migrate::reparse_legacy_choices(&app_state) {
                eprintln!("[choices reparse] {e}");
            }

            app.manage(app_state);

            // Watch lore/*.md and config/ for external edits.
            let handle = app.handle().clone();
            mirror::start_watcher(handle, data_dir)
                .map_err(|e| format!("启动文件监听失败: {e}"))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_worlds,
            commands::create_world,
            commands::delete_world,
            commands::list_stories,
            commands::create_story,
            commands::get_story,
            commands::delete_story,
            commands::set_story_mode,
            commands::list_lore_docs,
            commands::read_lore_doc,
            commands::save_lore_doc,
            commands::create_lore_doc,
            commands::delete_lore_doc,
            commands::open_lore_external,
            commands::list_passages,
            commands::get_state_card,
            commands::save_state_card,
            commands::get_summary,
            commands::reopen_turn,
            commands::build_bundle,
            commands::generate_stream,
            commands::cancel_generate,
            commands::submit_pasted_output,
            commands::apply_output,
            commands_ai::lore_ai_generate,
            commands_ai::create_lore_card,
            commands_config::read_config_file,
            commands_config::write_config_file,
            commands_config::test_connection,
            commands_lore::reset_config_to_defaults,
            commands_lore::get_world_event_background,
            commands_lore::save_world_event_background,
            commands_lore::get_story_event_background,
            commands_lore::save_story_event_background,
            commands_lore::list_lore_categories,
            commands_lore::create_lore_category,
            commands_lore::rename_lore_category,
            commands_lore::delete_lore_category,
            commands_lore::list_lore_cards,
            commands_lore::get_lore_card,
            commands_lore::update_lore_card,
            commands_lore::split_lore_doc_by_headings,
            commands_lore::import_lore_markdown,
            commands_lore::list_timeline_events,
            commands_lore::upsert_timeline_event,
            commands_lore::delete_timeline_event,
            commands_lore::set_card_timeline,
            commands_v2::get_story_settings,
            commands_v2::save_story_settings,
            commands_v2::get_app_settings,
            commands_v2::get_engines,
            commands_v2::get_policy,
            commands_v2::list_characters,
            commands_v2::upsert_character,
            commands_v2::set_character_importance,
            commands_v2::delete_character,
            commands_v3::list_turn_prompts,
            commands_v3::preview_bundle,
            commands_v3::get_director,
            commands_v3::save_director,
            commands_v3::list_state_history,
            commands_v3::undo_state,
            commands_v3::list_saves,
            commands_v3::create_save,
            commands_v3::restore_save,
            commands_v3::delete_save,
            commands_v3::list_lore_pins,
            commands_v3::set_lore_pin,
            clipboard::clipboard_read_text,
            clipboard::clipboard_write_text,
            clipboard::clipboard_peek,
            export::export_novel,
            export::open_in_shell,
            retrieval::retrieval_status,
            retrieval::rebuild_retrieval_index,
            retrieval::test_embedding,
            chapters::list_chapters,
            chapters::rename_chapter,
            search::search_story,
            dialogs::pick_file,
            dialogs::pick_save_path,
            dialogs::pick_folder,
            diagnostics::log_event,
            diagnostics::log_tail,
            diagnostics::export_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
