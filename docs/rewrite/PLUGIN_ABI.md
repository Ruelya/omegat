# 插件 ABI（冻结）

插件是 `cdylib`，导出：

```c
const char* omegat_plugin_abi(void);
```

返回 UTF-8 JSON，至少：

```json
{ "id": "example", "name": "Example", "version": "1.0.0", "kind": "filter" }
```

sidecar 用 `libloading` 加载目录内 `.so` / `.dylib` / `.dll`。失败隔离，不阻断项目打开。

开发者用 `crates/omegat-plugin` 的清单格式，勿链接 Java `IFilter`。
