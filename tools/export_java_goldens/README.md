# Export Java goldens

Goldens under `fixtures/goldens/` are produced by **running Java 6.2** in
`reference/java`. A field-check script is not an exporter.

## Export (requires JDK 21 and a Gradle cache)

```bash
cd reference/java
./gradlew exportGoldens --no-daemon
# honesty surfaces only (dialect tags, IEditor, menus, prefs, FilterTest inventory,
# HTMLFilter2Test-per-method goldens):
./gradlew exportGoldens --args="$(realpath ../../fixtures/goldens) honesty" --no-daemon
# engine + honesty (SRX / tokens / glossary / stats + the surfaces above):
./gradlew exportGoldens --args="$(realpath ../../fixtures/goldens) engine" --no-daemon
```

The task runs `org.omegat.tools.ExportGoldens`, which calls the same
`parseFile` / `translateFile` path as `TestFilterBase` and writes JSON under
`fixtures/goldens/`. The `honesty` / `engine` modes also write
`dialect_tags.json`, `ieditor_methods.json`, `menu_actions.json`,
`preference_keys.json`, `filter_tests.json`, and HTMLFilter2Test-per-method
goldens. Token cases for Lucene languages use that language’s text and
`NONE` + `GLOSSARY` + `MATCHING` — not Latin `"Hello worlds running"`.

CI does **not** run Gradle. It only checks that the committed goldens exist
and that `java_test` / `exported_by` look like a real export.

## Provenance check (no Java)

```bash
python3 tools/export_java_goldens/check_provenance.py
```

This script refuses files that lack a real `java_test` method name or that
still use `must_contain`.
