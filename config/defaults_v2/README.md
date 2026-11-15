# 配置说明

这是**便携式应用**（数据都在 exe 同目录的 `data/`），本目录 `data/config/` 里的文件是**活配置**，应用直接读取；界面上的设置面板写的也是这些文件。
可以直接用 VSCode / 记事本编辑，保存后应用会自动重新读取（YAML 解析失败时保留上一份可用配置并提示）。

| 文件 | 作用 |
| --- | --- |
| `engines.yaml` | 三种调用模式的 base_url / model / 采样参数 / 超时；填入你自己的 API key |
| `app.yaml` | 全局开关：`retrieval_enabled`、`event_background_soft_cap`、`fixed_background` |
| `policies/*.yaml` | 三种模式各自的上下文策略（模块开关、token 预算、检索 Top-N、节流） |
| `templates/system.*.md` | 三种模式的提示词骨架，用 `{{模块名}}` 占位 |
| `templates/output.*.md` | 三种模式的输出契约（一次返回正文/选项/状态增量/摘要增量；**每个选项必须独占一行**） |

模板占位符：`{{system_contract}}` `{{persona_opening}}` `{{state_card}}`
`{{rolling_summary}}` `{{lore_chunks}}` `{{recent_passages}}` `{{action}}`
`{{fixed_background}}` `{{event_background}}`。

`{{event_background}}` = 世界观事件背景（或剧本覆盖）＋ 勾选了「注入上下文」的时间线条目，
**全量注入、不走检索、不参与自动裁剪**；超过 `event_background_soft_cap` 只给警告。

设定正文不在这里，位于 `data/worlds/<世界观>/lore/*.md`，
可在「设定集」里用可视化表单或 Markdown 源码编辑，也可用外部编辑器打开（改完保存会自动同步并重切片）。
分类、标签、简介、时间线等元数据存 SQLite，不写进 `.md`。

## 升级时这些文件怎么处理

程序内置了 v1 出厂快照（`config/defaults_prev/`，仅开发用）和版本号 `defaults_version`：

- 你**没改过**的文件（`当前 sha == 旧版出厂 sha`）→ 自动更新为新版出厂值；
- 你**改过**的文件 → 保留你的版本，并生成 `<文件名>.new` 放新版内容供手工对照合并（设置 → 关于 会列清单）；
- 本 README 等说明文件 → 强制刷新，刷新前先备份到 `backups/`。

需要一键还原时，用「设置 → 关于 → 恢复出厂配置」：整个 `config/` 会先备份到 `backups/config-<时间>/`，再覆盖为出厂值。
