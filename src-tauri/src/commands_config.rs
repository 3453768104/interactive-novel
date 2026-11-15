//! 配置文件与连接测试。从 commands.rs 里拆出来，让回合流水线那个文件专注于
//! "生成 → 解析 → 写入"。

use crate::context;
use crate::engine;
use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use tauri::State;

#[tauri::command(rename_all = "snake_case")]
pub fn read_config_file(state: State<'_, AppState>, rel_path: String) -> CmdResult<String> {
    let st = &*state;
    context::read_config_text(st, &rel_path).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn write_config_file(
    state: State<'_, AppState>,
    rel_path: String,
    content: String,
) -> CmdResult<()> {
    let st = &*state;
    if rel_path.ends_with(".yaml") || rel_path.ends_with(".yml") {
        serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|e| format!("YAML 解析失败，未写入（原文件保持不变）: {e}"))?;
    }
    context::write_config_text(st, &rel_path, &content).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn test_connection(
    state: State<'_, AppState>,
    mode: String,
) -> CmdResult<String> {
    if mode == "paste" {
        return Ok("粘贴模式无需连接，直接复制提示词即可".into());
    }
    // Clone everything out of State before awaiting, so the future stays Send.
    let profile = {
        let st = &*state;
        engine::profile_for(st, &mode)
            .map_err(cmd_err)?
            .ok_or_else(|| format!("engines.yaml 中缺少 {mode} 模式配置"))?
    };
    drop(state);
    engine::test_connection(&profile).await.map_err(cmd_err)
}
