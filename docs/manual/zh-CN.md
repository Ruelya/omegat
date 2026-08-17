# OmegaT 用户手册

OmegaT 是键盘优先的计算机辅助翻译工作站。本手册对应 Rust + Electron 版本。

## 安装

- **源码：** 安装 Rust stable 与 Node.js 22，按仓库 `README.md` 操作。
- **Linux：** 解压 CI 产出的 `tar.gz`，或安装 `electron-builder` 生成的 `deb`/`rpm`。
- **Windows / macOS：** 使用 NSIS 或 DMG。CI 包默认未签名。

引擎是 sidecar 进程（`omegat-sidecar`）。渲染进程不直接读写项目磁盘。

## 新建或打开项目

1. 启动桌面程序。
2. **打开项目** 并选择含 `omegat.project` 的目录。
3. **新建项目** 设置语言对、根目录与是否分句。

Java 时代的项目目录无需转换即可打开。`omegat.project` 中的未知 XML 会原样保留。

## 翻译

- **Enter** 提交当前段并前进。
- **Ctrl/Cmd+I** 插入最佳模糊匹配。
- **Ctrl/Cmd+N** / **Ctrl/Cmd+P** 下一段 / 上一段。
- **Ctrl/Cmd+S** 保存；**Ctrl/Cmd+D** 编译到 `target/`。
- **Ctrl/Cmd+F** 搜索。

## 命令行

```bash
omegat translate <project>
omegat stats <project>
omegat --help
```

`--no-team` 跳过仓库同步。旧的 `--mode console-*` 仍然接受。

## 兼容说明

Java JAR 插件与 Groovy 脚本不再加载。新插件见 `docs/rewrite/PLUGIN_ABI.md`，脚本迁移见 `docs/rewrite/MIGRATION.md`。
