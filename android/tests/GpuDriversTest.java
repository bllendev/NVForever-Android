package org.nvforever.android;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/** Run directly with javac/java; these checks need neither Android nor game data. */
public final class GpuDriversTest {
    public static void main(String[] args) throws Exception {
        Path temp = Files.createTempDirectory("nvf-gpu-drivers-");
        File files = temp.toFile();
        try {
            metaJson();
            names();
            elfHeader();

            // Nothing installed: no folder, no choice, nothing to say about the last start.
            require(GpuDrivers.installed(files).isEmpty(), "an ordinary install has no driver");
            require(GpuDrivers.selected(files) == null, "and no choice");
            require(GpuDrivers.selectedLabel(files) == null, "and nothing to call it");
            require(GpuDrivers.lastStartFellBack(files) == null, "and no fallback to report");
            require(!GpuDrivers.root(files).exists(), "reading the choice creates no folder");

            // A driver zip in the emulator format: one library, one meta.json.
            String id = GpuDrivers.install(files, zip(
                "libvulkan_freedreno.so", elf(true),
                "meta.json", ("{\"schemaVersion\":1,\"name\":\"Turnip\",\"driverVersion\":\"25.0.3\","
                    + "\"libraryName\":\"libvulkan_freedreno.so\"}").getBytes(StandardCharsets.UTF_8)));
            require(id.equals("turnip-25.0.3"), "the folder is named after the driver: " + id);
            List<String[]> installed = GpuDrivers.installed(files);
            require(installed.size() == 1, "one driver is installed");
            require(installed.get(0)[1].equals("Turnip 25.0.3"), "meta.json names it: " + installed.get(0)[1]);
            require(GpuDrivers.selected(files) == null, "installing one does not choose it");

            // The same zip again is a second driver, not a folder written over.
            String second = GpuDrivers.install(files, zip("libvulkan_freedreno.so", elf(true),
                "meta.json", "{\"name\":\"Turnip\",\"driverVersion\":\"25.0.3\"}".getBytes(StandardCharsets.UTF_8)));
            require(!second.equals(id), "a second copy gets a folder of its own: " + second);
            require(GpuDrivers.installed(files).size() == 2, "both are installed");
            GpuDrivers.delete(files, second);
            require(GpuDrivers.installed(files).size() == 1, "and one can be removed again");

            // Choosing one, as the game reads it.
            File root = GpuDrivers.root(files);
            Files.write(new File(root, "renderer_starting").toPath(), new byte[0]);
            Files.write(new File(root, "last_fallback").toPath(), "the last start died".getBytes(StandardCharsets.UTF_8));
            GpuDrivers.rememberNativeLibDir(files, "/data/app/org.nvforever.android.odin/lib/arm64");
            GpuDrivers.select(files, id);
            String selection = new String(Files.readAllBytes(new File(root, "selected").toPath()), StandardCharsets.UTF_8);
            require(GpuDrivers.value(selection, "folder").equals(id), "the game is told the folder");
            require(GpuDrivers.value(selection, "library").equals("libvulkan_freedreno.so"), "and the library");
            require(GpuDrivers.value(selection, "label").equals("Turnip 25.0.3"), "and what to call it");
            require(GpuDrivers.selected(files).equals(id), "the choice reads back");
            require(GpuDrivers.selectedLabel(files).equals("Turnip 25.0.3"), "with its label");
            require(!new File(root, "renderer_starting").exists(), "a new choice clears the old start marker");
            require(GpuDrivers.lastStartFellBack(files) == null, "and the old fallback note");
            require(new File(root, "native_lib_dir").isFile(), "the library folder is left for the game");

            require(GpuDrivers.mode(files).equals(id), "the row reads as the driver itself");

            // The system driver again, and removing the chosen driver unchooses it.
            GpuDrivers.select(files, null);
            require(GpuDrivers.selected(files) == null, "System is no file at all");
            require(GpuDrivers.mode(files) == null, "and System is the row");

            // Automatic: an installed driver when there is one, the system driver otherwise.
            GpuDrivers.select(files, GpuDrivers.AUTOMATIC);
            require(GpuDrivers.mode(files).equals(GpuDrivers.AUTOMATIC), "Automatic is the row");
            require(id.equals(GpuDrivers.selected(files)), "and it resolved to the one driver");
            GpuDrivers.delete(files, id);
            require(GpuDrivers.installed(files).isEmpty(), "the driver is gone");
            require(GpuDrivers.mode(files).equals(GpuDrivers.AUTOMATIC), "the row is still Automatic");
            require(GpuDrivers.selected(files) == null, "which is now the system driver");
            // Installing one again is what Automatic was waiting for.
            String again = GpuDrivers.install(files, zip("libvulkan_freedreno.so", elf(true)));
            require(again.equals(GpuDrivers.selected(files)), "a first install resolves Automatic to it");
            require(GpuDrivers.label(new File(GpuDrivers.root(files), again)).equals(again),
                "a zip without meta.json is called after its folder");
            GpuDrivers.select(files, null);
            GpuDrivers.delete(files, again);
            require(GpuDrivers.selected(files) == null, "removing the chosen driver unchooses it");
            require(GpuDrivers.installed(files).isEmpty(), "and it is gone");

            // A zip whose entries are paths, and one that tries to climb out of the folder.
            String nested = GpuDrivers.install(files, zip(
                "turnip/lib/libvulkan_freedreno.so", elf(true),
                "../../../evil.so", "not a driver".getBytes(StandardCharsets.UTF_8)));
            File folder = new File(GpuDrivers.root(files), nested);
            require(new File(folder, "libvulkan_freedreno.so").isFile(), "every entry is flattened to its own name");
            require(!new File(files, "evil.so").exists(), "and nothing is written outside the driver's folder");
            require(new File(folder, "evil.so").isFile(), "the climb became a plain name inside it");
            require(GpuDrivers.libraryIn(folder).equals("libvulkan_freedreno.so"), "the one AArch64 library is the driver");
            GpuDrivers.delete(files, nested);

            // What is refused: a 32-bit library, junk, and an empty zip.
            refused(files, zip("libvulkan_freedreno.so", elf(false)), "a 32-bit driver");
            refused(files, zip("libvulkan_freedreno.so", "MZ not an elf at all".getBytes(StandardCharsets.UTF_8)), "junk");
            refused(files, zip(), "an empty zip");
            require(GpuDrivers.installed(files).isEmpty(), "a refused zip leaves nothing behind");
            File[] left = GpuDrivers.root(files).listFiles(f -> f.getName().startsWith(".incoming-"));
            require(left != null && left.length == 0, "and no staging folder");

            System.out.println("GpuDriversTest OK");
        } finally {
            Files.walk(temp).sorted(Comparator.reverseOrder()).map(Path::toFile).forEach(File::delete);
        }
    }

    /** meta.json's string fields, read without a JSON library. */
    private static void metaJson() {
        String json = "{\n  \"schemaVersion\": 1,\n  \"name\" : \"Turnip\",\n"
            + "  \"driverVersion\": \"25.0.3\",\n  \"description\": \"\",\n"
            + "  \"author\": \"a \\\"quoted\\\" name\",\n  \"minApi\": 27\n}";
        require("Turnip".equals(GpuDrivers.metaField(json, "name")), "a field with spaces round the colon");
        require("25.0.3".equals(GpuDrivers.metaField(json, "driverVersion")), "and the version");
        require(GpuDrivers.metaField(json, "description") == null, "an empty value is no value");
        require(GpuDrivers.metaField(json, "libraryName") == null, "an absent field is no value");
        require(GpuDrivers.metaField(json, "minApi") == null, "a number is not a string field");
        require("a \"quoted\" name".equals(GpuDrivers.metaField(json, "author")), "an escape is read");
        require(GpuDrivers.metaField(null, "name") == null, "no file is no value");
        require(GpuDrivers.metaField("not json at all", "name") == null, "junk is no value");
        require(GpuDrivers.metaField("{\"name\":", "name") == null, "and half a file is no value");
        // The field's name as somebody else's value must not be read as the field.
        require("Turnip".equals(GpuDrivers.metaField("{\"a\":\"\\\"name\\\"\",\"name\":\"Turnip\"}", "name")),
            "the field is found past a value that looks like it");
    }

    /** Folder names: what may be joined to the drivers folder, and what a driver is called. */
    private static void names() {
        require(GpuDrivers.plainName("libvulkan_freedreno.so"), "a file name is one");
        require(!GpuDrivers.plainName(".."), "a climb is not");
        require(!GpuDrivers.plainName("."), "nor the folder itself");
        require(!GpuDrivers.plainName(""), "nor nothing");
        require(!GpuDrivers.plainName(null), "nor no name");
        require(!GpuDrivers.plainName("sub/dir.so"), "nor a path");
        require(!GpuDrivers.plainName("sub\\dir.so"), "nor a Windows path");
        File nowhere = new File("no-such-folder-for-a-test");
        require(GpuDrivers.freeId(nowhere, "Turnip 25.0.3").equals("turnip-25.0.3"), "a label becomes a folder name");
        require(GpuDrivers.plainName(GpuDrivers.freeId(nowhere, "../../etc")), "a label that is a path cannot become one");
        require(GpuDrivers.freeId(nowhere, "..").equals("driver"), "nor can a label that is only a climb");
        require(GpuDrivers.freeId(nowhere, ".hidden").equals("hidden"), "a driver is not hidden by its name");
        require(GpuDrivers.freeId(nowhere, "").equals("driver"), "a driver with no name is still a driver");
        require(!GpuDrivers.freeId(nowhere, "Automatic").equals(GpuDrivers.AUTOMATIC),
            "and a driver called Automatic does not take the launcher's own word");
        require(GpuDrivers.freeId(nowhere, "x".repeat(80)).length() == 40, "and a very long name is cut");
    }

    /** The ELF header check, which is what refuses a driver at install rather than at start. */
    private static void elfHeader() throws IOException {
        Path temp = Files.createTempFile("nvf-gpu-driver-elf-", ".so");
        try {
            Files.write(temp, elf(true));
            require(GpuDrivers.isAarch64Elf(temp.toFile()), "an AArch64 shared object is one");
            Files.write(temp, elf(false));
            require(!GpuDrivers.isAarch64Elf(temp.toFile()), "a 32-bit ARM one is not");
            Files.write(temp, new byte[] { 0x7f, 'E', 'L', 'F' });
            require(!GpuDrivers.isAarch64Elf(temp.toFile()), "nor a file too short to say");
        } finally {
            Files.deleteIfExists(temp);
        }
    }

    /** An ELF header: 64-bit little-endian AArch64, or 32-bit ARM when {@code arm64} is false. */
    private static byte[] elf(boolean arm64) {
        byte[] head = new byte[64];
        head[0] = 0x7f;
        head[1] = 'E';
        head[2] = 'L';
        head[3] = 'F';
        head[4] = (byte) (arm64 ? 2 : 1);
        head[5] = 1;
        head[6] = 1;
        head[0x10] = 3; // ET_DYN
        head[0x12] = (byte) (arm64 ? 0xb7 : 0x28);
        return head;
    }

    /** A zip of {@code name, bytes} pairs, as the player's driver zip arrives. */
    private static ByteArrayInputStream zip(Object... entries) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ZipOutputStream out = new ZipOutputStream(bytes)) {
            for (int i = 0; i + 1 < entries.length; i += 2) {
                out.putNextEntry(new ZipEntry((String) entries[i]));
                out.write((byte[]) entries[i + 1]);
                out.closeEntry();
            }
        }
        return new ByteArrayInputStream(bytes.toByteArray());
    }

    private static void refused(File files, ByteArrayInputStream zip, String what) {
        try {
            GpuDrivers.install(files, zip);
            throw new AssertionError(what + " was installed");
        } catch (IOException expected) {
            require(expected.getMessage() != null && !expected.getMessage().isEmpty(),
                what + " is refused with words for the player");
        }
    }

    private static void require(boolean ok, String what) {
        if (!ok) {
            throw new AssertionError(what);
        }
    }
}
