# 从 OmegaT 6.2 Java 迁移

默认产品是 Rust sidecar + Electron。`reference/java/` 只作对照，不嵌入 JVM。

## 脚本

| Java | 等价 |
|---|---|
| Groovy / GraalJS | JavaScript（优先 Node；无 Node 时仍执行 `editor`/`project`/`glossary`/`console` 调用） |
| `project` | `getSourceLanguage`, `getTargetLanguage`, `save` / `saveProject`, `compileProject` |
| `editor` | `getCurrentTranslation`, `getCurrentSource`, `setTranslation`, `replaceEditText`, `insertText`, `gotoEntry`, `gotoNextUntranslatedEntry`, `commitAndDeactivate` |
| `glossary` | `addEntry(source, target, comment)`, `search` |
| `console` | `println` / `print` |
| `mainWindow` | `showStatusMessageRB` |
| `Core` | `getProject`, `getEditor`, `getGlossary`, `getMainWindow` |
| 事件 6 类 | `scripts/application_startup/` … `scripts/new_word/`（目录名小写） |
| 12 个快捷槽 | `scripts/slot01.js` … `slot12.js` |

样例：`scripts/examples/entry_activated.js`、`replace_current.js`。CLI：`omegat script path.js --project <dir>` 或 `omegat translate --script path.js`。

`entry_activated` 脚本调用 `editor.replaceEditText(...)` 会写回当前段译文。

## LanguageTool

不再内嵌 LT JAR。配置 HTTP `v2/check`。未启动时 Issues 显示降级原因。

## 插件

不再加载 Java JAR。使用 `omegat-plugin.toml` + `cdylib`。见 [PLUGIN_ABI.md](PLUGIN_ABI.md)。

## 过滤器 / 项目

`omegat.project`、`project_save.tmx`、过滤器掩码与 Java 6.2 兼容。PDF 只抽文本写 `.pdf.txt`。MED 包是 zip，解到项目树，不是把整个包当一个文件复制。
