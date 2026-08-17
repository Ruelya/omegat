# OmegaT 用户手册

OmegaT 是键盘优先的计算机辅助翻译工作站。本手册描述 Rust + Electron 构建。

## 安装

- **源码：** 安装 Rust stable 与 Node.js 22，见仓库 `README.md`。
- **Linux：** 解压 CI `tar.gz` 或安装 `electron-builder` 产出的 `deb`/`rpm`。
- **Windows / macOS：** 使用 NSIS 或 DMG。CI 包默认未签名。

引擎是 sidecar 二进制（`omegat-sidecar`）。桌面壳不直接读写项目文件。

## 项目

打开含 `omegat.project` 的目录，或新建项目（源/目标语言、分句）。Java 时代的项目目录无需转换。

标准文件夹：`source/`、`target/`、`omegat/project_save.tmx`、`tm/`（`auto/`、`enforce/`、`mt/`、`penalty-*`）、`glossary/glossary.txt`、`dictionary/`。

## 翻译

Enter 提交并前进；Ctrl/Cmd+I 插入最佳匹配；Ctrl/Cmd+N/P 下一段/上一段；Ctrl/Cmd+S 保存；Ctrl/Cmd+D 编译；Ctrl/Cmd+F 搜索替换。

匹配、术语、备注、评论、段属性、多译文、机器翻译、词典、问题面板均可操作并调用真实 RPC。

## 偏好

每一个 Java 偏好页都有对应表单：常规、外观、保存、编辑、匹配、视图、过滤器、分句、拼写、LanguageTool、词典、术语、MT、自动完成、External Finder、团队、插件。`prefs.set` 后重启仍生效。

## 命令行

`omegat translate|stats|pseudo|search|align|team|wiki|convert`，以及遗留 `--mode console-*`。`--tag-validation abort|warn` 在提交/编译路径生效。`--no-team` 跳过仓库同步。

## 团队 / 脚本 / 插件

同步为 prepare → rebase → commit。同段冲突保留两侧。脚本为 JavaScript（6 类事件 + 12 快捷槽）。插件为 `omegat-plugin.toml` + `cdylib`。LanguageTool 为外部 HTTP `v2/check`。PDF 只抽文本写 `.pdf.txt`（与 Java `PdfFilter` 相同）。

许可：GNU GPL v3+。
