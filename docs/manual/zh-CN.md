# OmegaT 用户手册

诚实缺口：完整产品行为以英文长手册 `docs/manual/en.md`（按 Java DocBook 目录）为准；本文件是短译。

OmegaT 是键盘优先的计算机辅助翻译工作站。本手册描述 Rust + Electron 构建。Java 时代的 HTML 手册仍在 `reference/java`（DocBook / `release/index.html`），打包后可从帮助打开。

## 安装

- **源码：** 安装 Rust stable 与 Node.js 22，见仓库 `README.md`。
- **Linux：** CI 产出未签名的 `deb` / `rpm` / `tar.gz` 与 `dir` 目录。
- **Windows：** 未签名 NSIS。
- **macOS：** 未签名 DMG。签名与公证在 CI 外由发行负责人完成。

引擎是 sidecar 二进制（`omegat-sidecar`）。桌面壳不直接读写项目文件。界面 41 种语言按 UI 键从 `Bundle_*.properties` 迁译；`ar` 为从右到左；原生菜单走同一套目录。

## 项目

打开含 `omegat.project` 的目录，或新建项目（源/目标语言、分句）。Java 时代的项目目录无需转换。

标准文件夹：`source/`、`target/`、`omegat/project_save.tmx`、`tm/`（`auto/`、`enforce/`、`mt/`、`penalty-*`）、`glossary/glossary.txt`、`dictionary/`。

## 翻译

段编辑器保护标签；空白 / NBSP / Bidi / 术语 / TM/MT 来源标记由偏好开关控制。Enter 提交并前进；Ctrl/Cmd+I 插入最佳匹配；模糊 1–5 有菜单加速键；Ctrl/Cmd+N/P 下一段/上一段；Ctrl/Cmd+S 保存；Ctrl/Cmd+D 编译；Ctrl/Cmd+F 搜索（精确/关键词/正则、笔记、评论、作者、日期、替换预览）。

九个停靠：编辑器、匹配、术语、词典、机器翻译、笔记、评论、多译文、段属性。文件列表与问题为独立窗口。布局会持久化。

## 偏好

25 个偏好页写入 sidecar 实际消费的键：常规、外观、字体、颜色、保存、编辑、匹配、视图、源文件、过滤器、分句、快捷键、拼写、LanguageTool、词典、术语、MT、自动完成（术语/自动文本/字符表/历史补全/历史预测）、External Finder、团队、安全存储、版本检查、插件。更改界面语言会重建原生菜单。`create` 译为「创建」，不是「添加」。

## 命令行

`omegat translate|stats|pseudo|search|align|team|script|wiki|convert`，以及遗留 `--mode console-*`、`--no-team`、`--alignDir`、`--tag-validation abort|warn` 等。`omegat --help` 列出全部旗标。

## 团队

四种仓库：file / HTTP（真下载）/ git / svn。解析并应用 `omegat.project` 的 mapping 与 include/exclude。工作副本在 `.repositories/<sanitized-url>/`。同步为 **prepare → rebase（TMX 与术语）→ commit/push**。同段冲突保留两侧，对话框提供保留我方 / 对方 / 手工。`--no-team` 保持本地。

## 对齐器

mALIGNa：HEAPWISE（过滤器抽段 + SRX + 长度 HMM，不是按空白切词）、PARSEWISE、ID。Viterbi 与 Forward-Backward 是不同算法。CHAR/WORD + Normal/Poisson。GUI 可合并/拆分/上移下移并导出 TMX。

## 脚本

绑定对象 `project` / `editor` / `glossary` / `console` / `mainWindow` / `Core` 的可调用方法与 Java 对等（当前段读写、插入/覆盖、跳转、保存、compile、术语增查、`console.println`）。6 类事件目录 + 12 槽。`--script`。不执行 Groovy，见 `docs/rewrite/MIGRATION.md`。

## 过滤器 / Wiki / MED

49 个 Java 过滤器类按 Dialect 与选项实现；JSON/CSV/Markdown 是额外格式。标签 QA：MISSING / EXTRANEOUS / ORDER / DUPLICATE / MALFORMED / ORPHANED / WHITESPACE。Wiki 从 MediaWiki XML 抽页到 `source/`。MED 是 zip，解包到项目树。

## 机器翻译 / Finder / 自动完成

7 个引擎按 Java 模块的 URL 与鉴权头实现；凭证进加密 prefs / OS keychain。录制夹具在 `fixtures/mt/<engine>/`。External Finder 兼容现有 finder XML。自动完成五类：术语、自动文本、字符表、历史补全、历史预测（下一词模型）、标签。

## LanguageTool / 拼写 / 词典

LanguageTool 为 HTTP `v2/check`。未配置 URL 时 Issues 出现 `severity=info` 降级项，禁止空列表假装干净。Hunspell 读真实 `.aff`/`.dic`。Lucene 与 Morfologik 走不同资源路径。StarDict（含 `.dict.dz`）与 DSL（含 `.dsl.dz`）。

## 插件

不加载 Java JAR。插件为 `omegat-plugin.toml` + 导出 `omegat_plugin_register` 的 cdylib，注册 Filter / MT / Tokenizer。示例插件必须出现在 `filters.list` 并能解析 `fixtures/plugin/sample.example`。见 `docs/rewrite/PLUGIN_ABI.md`。

## 帮助与许可

GNU GPL v3+。本 Markdown 手册随包装入（`docs/manual`）。`reference/java` 对照树在 STATUS 仍有需要时保留，不嵌入 JVM。
