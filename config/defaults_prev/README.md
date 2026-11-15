# 配置说明

这个目录里的文件是**活配置**，应用直接读取；界面上的设置面板写的也是这些文件。
可以直接用 VSCode / 记事本编辑，保存后应用会自动重新读取（YAML 解析失败时保留上一份可用配置并提示）。

| 文件 | 作用 |
| --- | --- |
| `engines.yaml` | 三种调用模式的 base_url / model / 采样参数 / 超时；填入你自己的 API key |
| `app.yaml` | 全局开关，含「设定检索」是否启用 |
| `policies/*.yaml` | 三种模式各自的上下文策略（模块开关、token 预算、检索 Top-N、节流） |
| `templates/system.*.md` | 三种模式的提示词骨架，用 `{{模块名}}` 占位 |
| `templates/output.*.md` | 三种模式的输出契约（要求模型一次返回正文/选项/状态增量/摘要增量） |

模板占位符：`{{system_contract}}` `{{persona_opening}}` `{{state_card}}`
`{{rolling_summary}}` `{{lore_chunks}}` `{{recent_passages}}` `{{action}}` `{{fixed_background}}`。

设定文档不在这里，位于 `%APPDATA%/interactive-novel/worlds/<世界观>/lore/*.md`，
可在应用内编辑，也可用外部编辑器打开（改完保存会自动同步并重切片）。
