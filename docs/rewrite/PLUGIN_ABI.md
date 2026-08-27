# 插件 ABI（冻结）

插件是 `cdylib`，放在配置目录 `plugins/<id>/` 或环境变量 `OMEGAT_PLUGINS_DIR`，带清单 `omegat-plugin.toml`：

```toml
id = "example"
name = "Example Filter"
version = "1.0.0"
plugin_type = "filter"
entry = "libomegat_example_plugin.so"
```

## 导出符号

```c
const char* omegat_plugin_abi(void);

typedef int (*omegat_filter_parse_fn)(const char* path, char* out_json, int cap);
typedef int (*omegat_filter_write_fn)(const char* src, const char* dest, const char* translations_json);
typedef int (*omegat_marker_fn)(const char* input_json, char* out_json, int cap);

typedef struct omegat_plugin_host {
    void* ctx;
    void (*register_filter)(void* ctx, const char* id, const char* name, const char* masks,
                            omegat_filter_parse_fn parse, omegat_filter_write_fn write);
    void (*register_mt)(void* ctx, const char* id, const char* name);
    void (*register_tokenizer)(void* ctx, const char* id, const char* name);
    /* ABI 结构只允许在尾部追加字段；旧插件仍可读取上面的稳定前缀。 */
    void (*register_marker)(void* ctx, const char* id, const char* name,
                            omegat_marker_fn mark);
} omegat_plugin_host;

void omegat_plugin_register(const omegat_plugin_host* host);
```

- `omegat_plugin_abi` 返回 UTF-8 JSON：`{ "id", "name", "version", "kind" }`。
- `omegat_plugin_register` **必须**调用至少一个 `register_filter` / `register_mt` /
  `register_tokenizer` / `register_marker`。只导出 ABI 字符串不算注册。
- `parse` 把段列表写成 JSON：`{"segments":[{"id":"0","source":"..."}]}`。
- `write` 的 `translations_json` 是 `{"0":"译文",...}`。返回 `0` 成功。
- `mark` 输入包含完整 `entry_key`（`file/source_text/id/prev/next/path`）、
  `source_text`、`translation_text`、`is_active` 等编辑器上下文；输出为
  `{"marks":[{"start_offset":3,"end_offset":9,"painter":"native-plugin",
  "entry_part":"TRANSLATION"}]}`。offset 是 UTF-16 单元，区间为半开区间。
  host 会拒绝越界/空区间、空 painter、非 UTF-8 或畸形 JSON。
- sidecar 用 `libloading` 加载。失败隔离，不阻断项目打开。
- 注册到的过滤器出现在 `filters.list`，并参与 `project.open` 抽段。`filters.parse` 可对单个文件试跑。
- 注册到的 Marker 出现在 `markers.list`；renderer 把它注册为异步 Marker，
  通过 `markers.query` 执行 cdylib，并沿用逐 EntryKey/逐 Marker request token
  丢弃编辑、remark 或卸载后的陈旧回调。

示例：`crates/omegat-example-plugin`（`*.example` 过滤器 + 查找 `plugin`
的原生 Marker，夹具 `fixtures/plugin/sample.example`）。

开发者用本 ABI，勿链接 Java `IFilter`。
