/**
 * Optional Java-side exporter. The committed JSON under fixtures/goldens/
 * already encodes the assertions from TestFilterBase subclasses.
 *
 * Compile against reference/java only when you need to dump a new format:
 *   javac -cp <omegat test classpath> ExportFilterGoldens.java
 *
 * Prefer updating the JSON from the Java test source rather than from Rust.
 */
public final class ExportFilterGoldens {
    private ExportFilterGoldens() {}

    public static void main(String[] args) {
        System.out.println("See tools/export_java_goldens/README.md");
        System.out.println("Goldens are transcribed from org.omegat.filters.*Test");
    }
}
