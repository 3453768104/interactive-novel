# 配置说明

这是**便携式应用**（数据都在 exe 同目录的 `data/`）。本目录 `data/config/` 里的文件是**活配置**，应用直接读取；「设置 → 策略面板」、源码 tab、外部编辑器写的都是同一份文件。
可以用 VSCode / 记事本编辑，保存后应用会自动重新读取（YAML 解析失败时保留上一份可用配置并提示）。

| 文件 | 作用 |
| --- | --- |
| `engines.yaml` | 三种调用模式的 base_url / model / 采样参数 / 超时；填入你自己的 API key |
| `app.yaml` | 全局开关：`stop_triggers` / `max_consecutive_turns` / `analysis_second_pass` / `character_inject` / `persona_soft_cap` / `opening_scene_turns` / `show_scene_markers` / `retrieval_enabled` / `event_background_soft_cap` / `fixed_background` |
| `policies/*.yaml` | 三种模式各自的上下文策略（模块开关、token 预算、trim 优先级、节流） |
| `templates/system.{api,local,paste}.md` | 主角视角提示词骨架 |
| `templates/system.omni.{api,local,paste}.md` | 全知读者视角提示词骨架（按剧本 `pov` 自动选用） |
| `templates/output.*.md` | 四段输出契约：NARRATION / ANALYSIS / STOP / CHOICES（**每个选项必须独占一行**） |

## 模板占位符

`{{protagonist}}`（主角人设，**不可裁剪**）`{{opening_scene}}`（仅前 N 回合）
`{{persona_opening}}`（前两者拼接，兼容旧模板）`{{characters}}` `{{state_card}}`
`{{rolling_summary}}` `{{lore_chunks}}` `{{recent_passages}}` `{{action}}`
`{{stop_policy}}` `{{system_contract}}` `{{fixed_background}}` `{{event_background}}`。

- `{{event_background}}` = 世界观事件背景（或剧本覆盖）＋ 勾选了「注入上下文」的时间线条目，**全量注入、不走检索、不参与自动裁剪**；超过 `event_background_soft_cap` 只给警告。
- `{{protagonist}}`、`{{event_background}}`、`{{fixed_background}}` 都绕过 `trim_to_budget`，保证 100% 进上下文；预算报告里仍会上报它们的 token 数。
- `ModuleCfg.trim` 数字越大，整块提示词超 cap 时越先被压缩（从旧正文、摘要开始，绝不碰主角人设）。
- `throttle.summary_every_n` / `throttle.state_mode`（`every_turn` / `every_n` / `manual`）现在真的会被 `apply_output` 读取。

## 四段输出契约

```
<<<NARRATION>>> 小说正文 <<<END_NARRATION>>>
<<<ANALYSIS>>>  {"world":{},"player":{},"characters":[],"summary":"","new_character_candidates":[]} <<<END_ANALYSIS>>>
<<<STOP>>>      {"stop":true,"reason":"","trigger":""} <<<END_STOP>>>
<<<CHOICES>>>   A. …（每行一个） <<<END_CHOICES>>>
```

- `stop=false` 表示继续写，此时不写 CHOICES 段，阅读器显示「继续阅读」。
- `ANALYSIS` 段被解析后结构化写入分层状态卡与角色表；缺失时仍兼容旧的 `STATE_DELTA` / `SUMMARY_DELTA` 标记（标记 + JSON 双通道、七层容错解析、人工修正）。
- `api` 模式优先接受单个 JSON 对象，字段为 `narration` / `analysis` / `stop` / `choices`。

设定正文不在这里，位于 `data/worlds/<世界观>/lore/*.md`，
可在「设定集」里用可视化表单或 Markdown 源码编辑，也可用外部编辑器打开（改完保存会自动同步并重切片）。
分类、标签、简介、时间线等元数据存 SQLite，不写进 `.md`。

## 升级时这些文件怎么处理

程序内置了**上一版出厂快照**（当前是 `config/defaults_v2/`，仅开发用）和版本号 `DEFAULTS_VERSION = 3`：

- 你**没改过**的文件（`当前 sha == 上一版出厂 sha`）→ 自动更新为 v3 出厂值；
- 你**改过**的文件 → 保留你的版本，并生成 `<文件名>.new` 放新版内容供手工对照合并（设置 → 关于 会列清单）；
- 新增文件（如全知视角模板）→ 直接写入；
- 本 README 等说明文件 → 强制刷新，刷新前先备份到 `backups/`。

需要一键还原时，用「设置 → 关于 → 恢复出厂配置」：整个 `config/` 会先备份到 `backups/config-<时间>/`，再覆盖为出厂值。
