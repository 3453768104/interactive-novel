# 互动小说 · Interactive Novel

[![release](https://img.shields.io/github/v/release/3453768104/interactive-novel?label=release&color=4f46e5)](https://github.com/3453768104/interactive-novel/releases/latest)
[![platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4)](https://github.com/3453768104/interactive-novel/releases/latest)
[![license](https://img.shields.io/badge/license-MIT-2f7d4f)](LICENSE)

> 没有大纲、不排剧情：AI 负责叙述，你负责做决定。

一个 Windows 桌面端的**无大纲互动小说**应用。AI 写一段正文 → 停下让你以**主角的身份**行动 → 你选一个选项（或自己写一句）→ 故事接着往下走。你代入主角，代替主角的大脑做决定。

- **本地/联网/网页粘贴三种模式**共用同一份历史，随时切换：本地跑 9B 小模型不花钱，联网用 API，或者干脆粘到网页 AI 里白嫖。
- **设定集、角色、状态、摘要、导演大纲、存档**全部围绕"长篇连载"设计，不是玩具 demo。
- **便携**：数据就在 exe 旁边的 `data/`，整个文件夹拷到 U 盘就能用，不写注册表、不需要安装。

> 平台：Windows 10/11（依赖系统 WebView2）。技术栈：Tauri 2 + React 18 + TypeScript + Rust + SQLite。

## 下载

到 [Releases](https://github.com/3453768104/interactive-novel/releases/latest) 下载 `interactive-novel-v1.0.0-win-x64.zip`，解压后双击 `interactive-novel.exe` 即可——**免安装**，数据生成在同级 `data/` 目录，整个文件夹拷到 U 盘就能带走。

首次启动若提示"已保护你的电脑"，点「更多信息 → 仍要运行」（个人作品未做代码签名，可以自己从源码编译核对）。

只想试试手感：模式选「网页粘贴」，**不需要装任何模型服务**，提示词会自动进剪贴板，你贴到网页 AI 再把回复复制回来。

- **小说式阅读器**：连续正文流、羊皮纸主题（可切纯白/深色）、字号/行距/页宽/字体/首行缩进/一键沉浸；回合号、模式、重开收进 hover 细标记；选项是无编号短句，正文旁不再有 JSON 编辑框。
- **四段输出契约**：一次调用返回 `NARRATION`（正文）/ `ANALYSIS`（分析）/ `STOP`（要不要停）/ `CHOICES`（选项）。分析段结构化写入**世界状态 / 玩家状态 / 角色状态**，不再往日志里堆文本。
- **分析阶段**：默认随正文一次返回、零额外往返；`api`/`local` 可选再跑一次便宜小模型做独立分析（默认关）。
- **主角 100% 注入**：人设是唯一不可裁剪模块（修掉了本地 10K 模式下人设被从头部裁掉的 bug）；开场情境只在前 3 回合注入。
- **角色两档**：路人 / 重要（"喜欢"已并入重要）。只有**重要**角色会以「名字（第几回合登场）：当前状态」进长期上下文；正文里角色名旁有小「＋」角标，点一下即提升。预设角色（设定集「角色」分类）默认是路人，等剧情真的引入或你手动标记才升级，同名角色只提升不新建。
- **可调停顿**：勾选「对话开始前 / 重大抉择 / 新角色登场 / 场景转换 / 写得够长」，模型据 `STOP` 段决定停不停；不停时正文接着写、显示「继续阅读」，你随时能「插入行动」打断；最大连写段数防止跑飞。
- **看小说（自动连写）**：工具栏「看小说」→ 选 3 / 5 / 10 回合或「一直写下去」。这些回合里提示词会替换掉"何时停下"的规则，模型**不给选项**、由它按人设替主角行动，写完自动接下一段；你随时点「停」。粘贴模式下也能用（回来会自动续上）。
- **双视角 + 暗场**：主角视角下模型**照样写**旁人的心思与别处的动静，但必须另起一段标 `〔暗场〕`——这类段落对读者隐藏（只喂给后续剧情与分析），所以配角状态会真实演化；全知读者视角直接显示，并在切换焦点处加「◇ 与此同时」。两种视角停下时给的仍是主角行动。阅读设置里有「显示暗场（作者视图）」。
- **策略 GUI**：设置里有可视化面板（模块开关、预算滑块、停顿条件、节流、分析开关、角色注入、视角相关开关）+ 实时预算预览；源码 tab 仍写同一份 YAML。
- **三种调用模式**（同一份历史、可随时切换）：`paste` 网页粘贴（零依赖、免费）/ `local` 任意 OpenAI-compatible 本地服务 / `api` 联网。
- **设定集**：事件背景（世界观一份 + 剧本可覆盖，全量注入不裁剪）＋ 分类树/搜索/卡片网格/时间线；可视化表单、Markdown 源码、外部编辑器三路并存。
- **设定 AI 辅助创作**：设定集 →「AI 生成」，可新建一张卡或扩写已有卡；输入一句话要求 + 篇幅 + 语气，生成草稿落进可编辑的预览框，改完再保存（新建写入新卡，扩写写回原卡）。沿用当前调用模式，粘贴模式会给提示词让你贴到网页 AI。生成结果绝不自动覆盖既有设定。
- **设定三路注入**（默认已开）：`lore_pins` 置顶卡整张注入（不受检索开关影响）/ `lore_index` 设定集目录（分类 + 标题，让模型知道"有这条设定"）/ `lore_chunks` **真 BM25 检索**——索引时把中文切成「单字 + 二元组」塞进 FTS5（绕开 unicode61 不分词的坑），`bm25()` 排序；配了 `embedding.model` 时再叠一路**向量余弦**（本地 LM Studio/Ollama 或任意 OpenAI-compatible 在线向量服务都行），两路用 **RRF** 融合。
- **导演模式**：① 长期大纲（`director_outline`，正常预算）② 本回合指令（`director_note`，整段注入、不裁剪、用后自动清空）。
- **存档 / 读档**：每回合自动存一份（留最近 60），也可手动命名；读档会删掉该回合之后的正文并还原状态卡/前情/角色，设定文档永远不动。
- **上下文可审计**：每回合的整段 prompt 落库，阅读页「上下文」抽屉能看真实预算表，也能在**不调模型**的情况下预览"下一回合会注入什么"。
- **导出为小说**：工具栏「导出」，可设书名、每章字数、是否含暗场，输出 **Markdown / EPUB / DOCX**（EPUB 自带目录，Word 可直接改）；默认剔除 `〔暗场〕`、回合号与选项。
- **目录与全文搜索**：按字数自动切章（场景切换可提前断章），目录里可改章名、点一下跳转；`Ctrl+F` 搜正文，结果按相关度排序并可跳回那一回合。
- **读小说的手感**：空格/→ 继续、`1–4` 选选项、`Esc` 停止、`Ctrl+Z` 撤销状态修改；退出再进恢复上次阅读位置。
- **状态可改可撤**：档案里世界状态/玩家状态可直接编辑（数组用「、」分隔），每次修改进历史，`Ctrl+Z` 或「撤销」按钮回退。
- **原生文件对话框**：导入设定集、导出小说、选数据目录都走系统对话框（rfd，不碰主线程）。
- **粘贴模式准全自动**：进工作区自动把提示词放进剪贴板；轮询系统剪贴板，你复制网页 AI 的回复后自动解析，解析干净就直接写入并跳回阅读页（不碰浏览器、不碰登录态）。

## 前置准备

- Node 20+ / npm
- Rust stable（`rustup` 装即可）
- Windows 10/11（WebView2 运行时通常已自带）

只用粘贴模式的话**不需要任何模型服务**。要用本地模式：

1. 启动 LM Studio / Ollama / llama.cpp server，加载任意 7B~14B 模型（Q4 量化即可）；
2. 开启 OpenAI-compatible server（LM Studio 默认 `http://127.0.0.1:1234/v1`，Ollama 是 `http://127.0.0.1:11434/v1`）；
3. 「设置 → 连接配置」填 `base_url` 与模型名，点「测试连接」。

> 10K 上下的 KV cache 对 8GB 显存有压力；显存紧张时在策略面板下调 `context_window`，或把 `recent_passages` 的预算调小。

联网模式：在「连接配置 → 联网 API」填 `api_key`（明文只存在便携目录里，不会上传）。

## 运行

```bash
npm install
npm run tauri dev      # 开发模式：数据在 src-tauri/target/debug/data/
npm run tauri build    # 产出 src-tauri/target/release/interactive-novel.exe
```

**分发方式**：把 `interactive-novel.exe` 和它旁边的 `data/` 一起复制走。不生成 MSI/NSIS（装到 Program Files 会不可写）。

- 首次启动会检测旧版 `%APPDATA%/interactive-novel/` 是否有数据；有的话，弹出**置顶的 Windows 原生对话框**：【是】复制旧数据（含 `novel.db` + `-wal` + `-shm` 三件套），【否】从零开始；原目录不动，只问一次（`.migrated_from` / `.migration_skipped` 标记）。
  > 刻意不用 Tauri dialog 插件：它的 `blocking_show()` 用 `sync_channel` 阻塞等待、明确禁止主线程调用，而 `setup()` 正是主线程——会卡在弹窗导致 WebView 不加载、窗口纯黑。改用同步 `MessageBoxW`（自带消息循环）。
- `NOVEL_DATA_DIR` 环境变量可覆盖数据目录（dev 调试 / 特殊部署）。
- exe 放只读目录时启动给出明确提示，不会白屏。

## 快速上手

1. **世界观** → 填名称创建（自动生成空设定文档）。
2. **设定集** → 写「事件背景」；用分类树 + 卡片管理设定（可视化表单 / Markdown 源码）；「时间线」记世界历史年表。
3. **剧本** → 填主角姓名、人设、开场情境，选调用模式**与叙事视角**；可在列表单独覆盖事件背景。
4. **阅读** → 顶部工具条切视角 / 模式，右侧「档案」抽屉看世界状态、玩家状态、角色、前情、人设；「阅读设置」调排版。
5. 点选项（或「自己写一句」）推进；模型判断不该停时显示「继续阅读 →」，随时可「插入行动」。
6. 粘贴模式会跳到「粘贴工作区」：提示词**已自动复制**，你贴进网页 AI；回完全选复制，应用会自己接住并解析，干净就直接写入。格式被破坏时进「人工修正」（里面也能改 STOP 与分析段）。
7. 「导演」抽屉写长期大纲与本回合指令；「存档」抽屉随时存点/读档；「导出」把正文导成一本 Markdown 小说。
8. 不想动手时点工具栏「看小说」，选几回合让它自己往下写，你只管读。

「设置 → 上下文预览」可在**不调用任何 AI** 的情况下看整块提示词、各模块 token 预算与警告。

## 配置在哪

```
data/
├── novel.db                 # SQLite（含 FTS5 虚表 + v2 旁挂表）
├── config/                  # 活配置：面板和外部编辑器写同一份
│   ├── engines.yaml  app.yaml
│   ├── policies/{api,local,paste}.yaml
│   └── templates/
│       ├── system.{api,local,paste}.md            # 主角视角
│       ├── system.omni.{api,local,paste}.md      # 全知读者视角
│       └── output.{api,local,paste}.md           # 四段输出契约
├── worlds/<世界观>/
│   ├── lore/*.md            # 设定正文（1 个 .md = 1 张卡片）
│   └── stories/<剧本>/{story.md,state.md,summary.md}
└── backups/
```

| 文件 | 作用 |
| --- | --- |
| `engines.yaml` | 三种模式的 base_url / model / 采样参数 / 超时 / embedding |
| `app.yaml` | `stop_triggers`、`max_consecutive_turns`、`analysis_second_pass`、`character_inject`（`notable`/`none`）、`persona_soft_cap`、`opening_scene_turns`、`show_scene_markers`、`retrieval_enabled` 等 |
| `policies/*.yaml` | 各模式模块开关 / token 预算 / trim 优先级 / 节流（现在真的生效） |
| `templates/system.*.md` | 提示词骨架（`{{protagonist}}`、`{{characters}}`、`{{stop_policy}}` 等占位） |
| `templates/output.*.md` | 四段输出契约（每个选项必须独占一行） |

**推荐用「设置 → 策略面板」改这些值**：开关/滑块/下拉，保存时按行替换同一份 YAML，注释保留；源码 tab 与外部编辑器改的也是同一份文件，不存在双源。YAML 保存前会校验，解析失败不会覆盖原文件。

**出厂配置升级**：用 `config/defaults_v3/`（上一版快照）做三方 sha 比对——没改过的文件自动升级，改过的保留并写 `<文件名>.new` 供手工合并（设置 → 关于 会列出来）；README 类说明强制刷新并先备份。「恢复出厂配置」先备份整个 `config/`。当前 `DEFAULTS_VERSION = 4`。

详见 `config/defaults/README.md`。

## 设定检索怎么开 / 怎么关

v4 起**默认已经开着**，不用再手动建索引——设定卡导入时就已经切好片段（`lore_chunks`），检索只是按关键词挑几段塞进上下文。要改就在「设置 → 策略面板 → 设定集检索」：

| 开关 | 位置 | 作用 |
|---|---|---|
| 启用检索 | `app.yaml` 的 `retrieval_enabled` | 总闸。关掉 = 不按关键词捞正文 |
| 本模式注入命中片段 | `policies/<mode>.yaml` 的 `modules.lore_chunks.on` | 分模式闸门，`budget` 给额度、`top_n` 给条数 |
| 建索引 / 向量 | 面板同卡「建立 / 更新索引」按钮 | 建 FTS5 词项；配了 embedding 模型就顺带算向量 |
| 设定集目录 | `modules.lore_index.on` | 分类 + 标题清单，**不受总闸影响**；关掉模型就只认识置顶卡 |

手改 YAML 等价写法：

```yaml
# app.yaml
retrieval_enabled: true

# policies/local.yaml
modules:
  lore_index:  { on: true, budget: 1000, trim: 9 }
  lore_pins:   { on: true, budget: 600,  trim: 8 }
  lore_chunks: { on: true, budget: 900,  top_n: 3, trim: 6 }
```

三个模式各有一份策略，改哪个模式就改哪个文件（面板顶部有模式切换）。

**怎么确认真的注入了**：阅读页右上「上下文」→ 看预算表里 `设定集目录` / `设定检索` 两行的「实际」是不是 0；再用下面的输入框「预览下一回合」，换几个关键词看命中段数变化。零命中时预算表会直接给告警。

**想让某张卡永远在场**：设定集卡片右下角的 **★**（置顶）。置顶卡整张原文注入，不受检索开关与命中率影响——这是"重要设定必须每回合都在"的可靠做法。

**索引与向量**：BM25 的词项在设定切片时自动维护，启动时也会自愈补齐（`lore_fts` 与 `lore_chunks` 对齐）。向量那一路要 `engines.yaml` 里有 `embedding.model`（默认指向 LM Studio 的 `nomic-embed-text`）：

```yaml
embedding:
  provider: local
  base_url: http://127.0.0.1:1234/v1
  model: nomic-embed-text-v1.5.Q4_K_M
```

然后在策略面板点「建立 / 更新索引」——它会对每段切片调 `/embeddings`，把归一化向量写进 `chunk_vectors`。词表一变（换模型）会自动重算。没有 embedding 服务、或 `vec_topk: 0` 时，检索自动退化为纯 BM25，RRF 对单路同样成立；embedding 服务挂掉会退 60 秒冷却，不会每回合卡一次。

**想彻底关掉**：把前两个勾取消即可；目录也可以单独关。额度会自动回流给最近正文。

## 路线图

**v1（已完成）**：三模式闭环、四段输出契约与容错解析、小说式阅读器、分层状态卡与角色两档、双视角与暗场、可调停顿与自动连写、导演模式、存档/读档、上下文审计与预览、设定集（事件背景 + 时间线 + 卡片 + AI 辅助创作）、三路设定注入（置顶/目录/BM25 + 可选向量 RRF）、章节与全文搜索、导出 Markdown/EPUB/DOCX、粘贴模式准全自动。

**接下来（按优先级）**：
1. 长剧本性能：回合按需加载 + 阅读器窗口化、`story.md` 增量追加。
2. 数据安全：数据库自动备份、世界观/剧本 zip 导入导出。
3. 生成韧性：失败可续写、`retries` 真正生效、超窗硬截断。
4. 工程：拆分剩余的两个大文件、前端生成状态机 hook 化、金文件测试。

详见 [`docs/升级优化建议.md`](docs/升级优化建议.md)。

**明确不做**：账户体系、云同步、多人协作、浏览器自动化、网页接口逆向。

## 可维护性

- 后端按职责分文件：`commands.rs`（回合流水线）/ `commands_lore.rs` / `commands_v2.rs` / `commands_v3.rs` / `commands_config.rs`，检索、章节、搜索、导出、诊断各自独立模块。
- `data/logs/app-YYYY-MM-DD.jsonl` 记录生成耗时、prompt 估算、解析告警、导出动作；「设置 → 关于 → 导出诊断包」打包日志 + 脱敏配置 + 库统计（不含正文）。
- 配置里写了但不会被读取的键会在「关于」页列出来（比如拼错的键名）。
- `.github/workflows/ci.yml`：`tsc` + `vite build` + `cargo clippy` + `cargo test`。
- 测试 51 项，覆盖解析、上下文预算、检索（分词/BM25/RRF）、章节切分、正文搜索、导出（含 EPUB/DOCX 结构）、存档快照。

## 已知限制

- 事件背景 + 时间线 + 主角人设**全量注入、不裁剪**；写太长会挤占本地 10K 窗口。超过 `event_background_soft_cap`（2000）/ `persona_soft_cap`（800）只在预览里警告，请手动精简。
- 检索的 BM25 用「单字 + 二元组」分词，不是 jieba 那种词典分词：专有名词能命中，但换个说法仍可能漏；向量那一路需要本机有 embedding 模型并手动建一次索引，否则只有 BM25。关键场景仍建议手动 ★ 置顶。
- 中文常用字已做停用（的了是在…），所以单字查询偶尔返回空，用双字以上更稳。
- 目录受预算限制（本地约 1000 tokens），条目多的设定集只会列出一部分并注明"还有 N 条未列出"。
- 一张卡片 = 一个 `.md`；长文档可用卡片上的「拆分」按 `##` 拆成多张（原文件先备份）。表单改标题/分类/标签不会改 `.md` 里的 `# 标题`——标题权威在数据库。
- `state.md` 仍是可手改的 JSON（现在是 `{world:{}, player:{}, ...}` 分层结构），不是分节 Markdown；外部手改会回灌并升 revision。
- 世界观「角色」分类下的卡片在启动时播种为预设角色（默认 `passerby`、不进上下文）；手动删除后重启会再次出现（如需彻底移除，把该卡片移出「角色」分类）。
- 前端单包较大（CodeMirror 占大头），不影响本地使用。
- 只有 Windows 构建；未适配 macOS / Linux。
- 主要按单人自用打磨，多人协作/云端能力不在范围内。

## 项目结构

```
.
├── src/                     前端（React + TS）
│   ├── pages/               世界观 / 剧本 / 设定集 / 阅读 / 粘贴 / 设置
│   ├── components/          阅读器抽屉、策略面板、设定编辑、AI 对话框…
│   ├── reader/              阅读偏好、流式清洗、YAML 行级补丁
│   └── styles.css           设计令牌 + 组件样式 + 响应式
├── src-tauri/               后端（Rust）
│   ├── src/commands*.rs     命令层（回合流水线 / 设定 / 策略 / 配置 / AI）
│   ├── src/context.rs       上下文组装与预算
│   ├── src/engine.rs        输出解析、SSE 流式、一次性补全
│   ├── src/retrieval.rs     分词 + FTS5 BM25 + 向量 + RRF
│   ├── src/{chapters,search,export,diagnostics}.rs
│   └── src/db/schema.sql    幂等 schema（旁挂表，随时加）
├── config/defaults/         出厂配置与模板（版本化升级，可被用户覆盖）
└── docs/                    升级建议、界面说明、AI 创作说明、UI 预览
```

## 贡献

欢迎 issue / PR。提交前请确保：

```bash
npx tsc --noEmit && npm run build
cd src-tauri && cargo test --no-default-features
```

改动涉及 `config/defaults/` 时，请先把当前默认文件冻结成 `config/defaults_vN/` 快照并提升 `DEFAULTS_VERSION`，否则老用户的"未改动文件"会被误判成手工修改。

## 许可

[MIT](LICENSE)。第三方作品（例如某些小说的同人设定集）请勿随仓库分发；本地示例数据已通过 `.gitignore` 排除。
