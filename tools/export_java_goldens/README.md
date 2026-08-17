# Export Java goldens

Goldens under `fixtures/goldens/` are transcribed from Java 6.2 unit tests in
`reference/java/src/test/java/org/omegat/filters` and the matching fixtures.

## Regenerating from this tree

```bash
python3 tools/export_java_goldens/export.py
```

The script does **not** compile OmegaT. It copies the Java test assertions
(segment lists, options, write-back expectations) into JSON so Rust CI can
run without Gradle. When a Java test is the source of truth, the JSON
`java_test` field names the method.

To re-export by running Java (optional, needs a full Gradle cache):

```bash
cd reference/java && ./gradlew :test --tests org.omegat.filters.TextFilterTest
```

Then update the JSON by hand from the assertion values. Do not invent
segment lists from the Rust parser.
