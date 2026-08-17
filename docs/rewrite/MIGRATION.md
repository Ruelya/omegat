# Migration from Java OmegaT

## Projects

Copy the project folder as-is. `omegat.project`, `project_save.tmx`, `source/`, `target/`, `tm/`, and `glossary/` are read by the Rust engine. Unknown XML nodes in `omegat.project` (including `repositories`) are kept.

## Plugins

Java JARs listed in the historic `Plugins.properties` are **not** loaded.

Replace a JAR with a directory that contains `omegat-plugin.toml` (or JSON) and a `cdylib` / helper process. Types stay the same: `filter`, `tokenizer`, `marker`, `mt`, `glossary`, `dictionary`, `theme`, `repository`, `spell`, `language`, `misc`. See `PLUGIN_ABI.md`.

## Scripts

Groovy is not executed and the JVM is not embedded.

| Historic | Replacement |
|---|---|
| `scripts/*.groovy` | `scripts/js/<event>/*.js` |
| ApplicationEvent / ProjectEvent / … | Same event directory names |
| `project` / `editor` / `glossary` / `console` bindings | Passed into the JS hook as a JSON payload |

A sample hook lives at `scripts/js/entry_activated/log.js`. CLI: `omegat translate --script path/to/file.js`.

## LanguageTool

Do not drop a LanguageTool JAR on the classpath. Run LT as a separate process or HTTP service and set its URL in Preferences → LanguageTool. If the service is down, the editor degrades without blocking saves.

## Machine translation credentials

Secure Store / Jasypt values are not imported automatically. Re-enter API keys; they are stored in the OS keychain or encrypted preferences.

## Team

Git/SVN/HTTP/file mappings in `repositories` still apply. SVN uses the system `svn` binary. Use `--no-team` for a local-only session.
