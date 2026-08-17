# 从 OmegaT 6.2 Java 迁移

默认产品是 Rust sidecar + Electron。`reference/java/` 只作对照，不嵌入 JVM。

## 脚本

| Java | 等价 |
|---|---|
| Groovy / GraalJS | JavaScript（Node `eval` 或嵌入引擎） |
| `project` / `editor` / `glossary` / `console` / `mainWindow` | 同名绑定，见 `omegat-script` |
| 事件 `APPLICATION_STARTUP` 等 6 类 | `event:` 前缀文件名或 `omegat.events` |
| 12 个快捷槽 | `scripts/slot01.js` … `slot12.js` 或偏好 `script.slot.N` |

样例从 `reference/java/scripts/` 迁到 `scripts/examples/`。

## LanguageTool

不再内嵌 LT JAR。配置 HTTP `v2/check`（LanguageTool 独立服务）。未启动时 Issues 显示降级原因。

## 插件

不再加载 Java JAR。使用 `omegat-plugin.toml` + `cdylib`（`omegat_plugin_abi()`）。见 [PLUGIN_ABI.md](PLUGIN_ABI.md)。

## 过滤器 / 项目

`omegat.project`、`project_save.tmx`、过滤器掩码与 Java 6.2 兼容。PDF 只抽文本写 `.pdf.txt`（与 Java `PdfFilter` 相同边界）。
