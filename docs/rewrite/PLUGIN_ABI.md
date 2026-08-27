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

typedef struct omegat_plugin_host {
    void* ctx;
    void (*register_filter)(void* ctx, const char* id, const char* name, const char* masks,
                            omegat_filter_parse_fn parse, omegat_filter_write_fn write);
    void (*register_mt)(void* ctx, const char* id, const char* name);
    void (*register_tokenizer)(void* ctx, const char* id, const char* name);
} omegat_plugin_host;

void omegat_plugin_register(const omegat_plugin_host* host);
```

- `omegat_plugin_abi` 返回 UTF-8 JSON：`{ "id", "name", "version", "kind" }`。
- `omegat_plugin_register` **必须**调用 `register_filter` / `register_mt` / `register_tokenizer`。只导出 ABI 字符串不算注册。
- `parse` 把段列表写成 JSON：`{"segments":[{"id":"0","source":"..."}]}`。
- `write` 的 `translations_json` 是 `{"0":"译文",...}`。返回 `0` 成功。
- sidecar 用 `libloading` 加载。失败隔离，不阻断项目打开。
- 注册到的过滤器出现在 `filters.list`，并参与 `project.open` 抽段。`filters.parse` 可对单个文件试跑。

示例：`crates/omegat-example-plugin`（`*.example`，夹具 `fixtures/plugin/sample.example`）。

开发者用本 ABI，勿链接 Java `IFilter`。
