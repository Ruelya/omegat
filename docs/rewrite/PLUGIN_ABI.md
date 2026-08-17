# Plugin ABI (frozen in P9)

Java JAR plugins are **not** loaded.

A plugin is a directory containing `omegat-plugin.toml` (or JSON with the same fields):

```toml
id = "demo-filter"
name = "Demo"
version = "1.0.0"
plugin_type = "filter"
entry = "libdemo.so"
```

`plugin_type` is one of: `filter`, `tokenizer`, `marker`, `mt`, `glossary`, `dictionary`, `theme`, `repository`, `spell`, `language`, `misc`.

P0–P8 register built-in plugins in-process. External `cdylib` loading is the extension point; the manifest schema will not change without a major version bump.
