package org.nvforever.android;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * The Vulkan drivers the player installed themselves, and which of them the next game start
 * uses. NVForever ships no driver: the player supplies a driver zip in the emulator format (a
 * single .so, optionally with meta.json naming it), and the game opens it through libadrenotools
 * ({@code engine/nvforever/src/odin/gpu_driver.rs}, docs/odin-drivers.md).
 *
 * <p>Everything lives in the app's private storage, which is what libadrenotools requires: a
 * driver on shared storage could be rewritten by any app, so dlopen refuses it.
 *
 * <pre>
 * files/gpu_drivers/&lt;id&gt;/           one installed driver: its .so, and meta.json if the zip had one
 * files/gpu_drivers/selected        the chosen driver; absent means the system driver
 * files/gpu_drivers/mode            the launcher's own row: "automatic", else absent
 * files/gpu_drivers/renderer_starting  written by the game before the renderer starts, removed
 *                                   by the engine after 300 drawn frames
 * files/gpu_drivers/last_fallback   why the last start did not use the chosen driver
 * files/gpu_drivers/native_lib_dir  ApplicationInfo.nativeLibraryDir, for the game's fallback
 * </pre>
 *
 * <p>The engine reads {@code selected} and nothing else, so <b>Automatic</b> is a launcher
 * word: it resolves, every time it is chosen, to an installed driver if there is one and to the
 * system driver if there is not. NVForever bundles no driver, so on a fresh install Automatic
 * is exactly the system driver.
 *
 * <p>With nothing installed and nothing selected the game behaves exactly as it did before this
 * existed: no file is there, so none is read.
 *
 * <p>This class is plain Java over {@link File} and an {@link InputStream}, so it is compiled
 * and tested without an Android platform ({@code android/tests/GpuDriversTest.java}); the
 * launcher's page passes in {@code getFilesDir()} and opens the picked zip ({@link
 * OdinLauncher}).
 */
final class GpuDrivers {
    /** The folder below the app's files folder, as the engine's {@code gpu_driver::DIR}. */
    static final String DIR = "gpu_drivers";

    /**
     * The sentinel {@link #select} and {@link #mode} use for Automatic: an installed driver
     * when there is one, the system driver when there is not. Never a folder name: the ids
     * {@link #install} makes are derived from a driver's own label and never start with a dot.
     */
    static final String AUTOMATIC = "automatic";

    private static final String SELECTED = "selected";
    private static final String MODE = "mode";
    private static final String MARKER = "renderer_starting";
    private static final String LAST_FALLBACK = "last_fallback";
    private static final String NATIVE_LIB_DIR = "native_lib_dir";
    private static final String META = "meta.json";

    /** A driver zip is a library and a little text; anything larger is not one. */
    private static final long MAX_UNPACKED_BYTES = 256L * 1024 * 1024;
    private static final int MAX_ENTRIES = 64;
    /** ELF, 64-bit, little-endian, AArch64: a file that is not this cannot be a driver here. */
    private static final byte[] ELF64_LE = { 0x7f, 'E', 'L', 'F', 2, 1 };
    private static final int EM_AARCH64 = 0xb7;

    private GpuDrivers() {
    }

    // ---- what is installed ------------------------------------------------------------

    /** The drivers folder itself, which is absent until one is installed. */
    static File root(File filesDir) {
        return new File(filesDir, DIR);
    }

    /** The installed drivers as {id, label}, by label. Empty when none is installed. */
    static List<String[]> installed(File filesDir) {
        List<String[]> drivers = new ArrayList<>();
        File[] folders = root(filesDir).listFiles();
        if (folders == null) {
            return drivers;
        }
        for (File folder : folders) {
            if (!folder.isDirectory() || folder.getName().startsWith(".")) {
                continue;
            }
            if (libraryIn(folder) == null) {
                continue;
            }
            drivers.add(new String[] { folder.getName(), label(folder) });
        }
        drivers.sort((a, b) -> a[1].compareToIgnoreCase(b[1]));
        return drivers;
    }

    /** The chosen driver's id, or null for the system driver (including a stale choice). */
    static String selected(File filesDir) {
        String folder = value(read(new File(root(filesDir), SELECTED)), "folder");
        if (folder == null || !plainName(folder)) {
            return null;
        }
        File dir = new File(root(filesDir), folder);
        return dir.isDirectory() && libraryIn(dir) != null ? folder : null;
    }

    /** What to call the chosen driver, or null when the system driver is chosen. */
    static String selectedLabel(File filesDir) {
        String id = selected(filesDir);
        return id == null ? null : label(new File(root(filesDir), id));
    }

    /**
     * Which row of the Graphics driver choice is filled in: {@link #AUTOMATIC}, null for the
     * system driver, or an installed driver's id. This is the player's own choice, which is not
     * always the folder the next start opens: Automatic with nothing installed is the system
     * driver, and the row still reads Automatic.
     */
    static String mode(File filesDir) {
        String stored = read(new File(root(filesDir), MODE));
        if (stored != null && AUTOMATIC.equals(stored.trim())) {
            return AUTOMATIC;
        }
        return selected(filesDir);
    }

    /**
     * Chooses a driver for the next game start: {@link #AUTOMATIC}, a driver's id, or null for
     * the system driver (which is also what an id naming nothing installed means). Either way
     * the previous start's marker and fallback note go: the question they answer was about the
     * choice being replaced.
     */
    static void select(File filesDir, String idOrNull) {
        if (AUTOMATIC.equals(idOrNull)) {
            List<String[]> drivers = installed(filesDir);
            apply(filesDir, drivers.isEmpty() ? null : drivers.get(0)[0]);
            // Written after the choice, so a crash in between leaves the plainer of the two
            // answers: whatever `selected` says, read as a direct choice.
            write(new File(root(filesDir), MODE), AUTOMATIC + "\n");
            return;
        }
        new File(root(filesDir), MODE).delete();
        apply(filesDir, idOrNull);
    }

    /**
     * Remembers {@code ApplicationInfo.nativeLibraryDir} for the game, which reads it only when
     * it cannot ask the linker where its own libraries were extracted to.
     */
    static void rememberNativeLibDir(File filesDir, String dir) {
        if (dir == null || dir.isEmpty()) {
            return;
        }
        File root = root(filesDir);
        root.mkdirs();
        write(new File(root, NATIVE_LIB_DIR), dir + "\n");
    }

    /** Writes the folder the next start opens, or removes the file for the system driver. */
    private static void apply(File filesDir, String idOrNull) {
        File root = root(filesDir);
        File chosen = idOrNull != null && plainName(idOrNull) ? new File(root, idOrNull) : null;
        String library = chosen != null && chosen.isDirectory() ? libraryIn(chosen) : null;
        root.mkdirs();
        new File(root, MARKER).delete();
        new File(root, LAST_FALLBACK).delete();
        if (library == null) {
            new File(root, SELECTED).delete();
            return;
        }
        write(new File(root, SELECTED), "# Written by the launcher; applied when starting the game\n"
            + "folder " + idOrNull + "\n"
            + "library " + library + "\n"
            + "label " + oneLine(label(chosen)) + "\n");
    }

    /** Why the last game start did not use the chosen driver, or null when it did. */
    static String lastStartFellBack(File filesDir) {
        String why = read(new File(root(filesDir), LAST_FALLBACK));
        if (why == null) {
            return null;
        }
        why = why.trim();
        return why.isEmpty() ? null : why;
    }

    // ---- installing and removing ------------------------------------------------------

    /**
     * Unpacks a driver zip into files/gpu_drivers/&lt;id&gt;/ and returns the id. The zip must
     * hold exactly one AArch64 .so; meta.json, if there is one, names the driver and may name
     * which library to load. Folders inside the zip are flattened, so no entry can write
     * outside the driver's own folder. Nothing is selected: that is {@link #select}.
     *
     * @throws IOException if the zip cannot be read, holds no usable driver, or is too large
     */
    static String install(File filesDir, InputStream zip) throws IOException {
        File root = root(filesDir);
        if (!root.isDirectory() && !root.mkdirs()) {
            throw new IOException("Cannot create " + root);
        }
        File staging = new File(root, ".incoming-" + System.nanoTime());
        if (!staging.mkdirs()) {
            throw new IOException("Cannot create a folder for the driver");
        }
        try {
            unpack(zip, staging);
            String library = libraryIn(staging);
            if (library == null) {
                throw new IOException("That zip holds no 64-bit ARM Vulkan driver (one .so is expected)");
            }
            String id = freeId(root, label(staging));
            File target = new File(root, id);
            if (!staging.renameTo(target)) {
                throw new IOException("Cannot store the driver as " + id);
            }
            staging = null;
            // Automatic means "an installed driver if there is one", so a first install is what
            // it has been waiting for. A direct choice is left exactly as the player made it.
            if (AUTOMATIC.equals(mode(filesDir))) {
                select(filesDir, AUTOMATIC);
            }
            return id;
        } finally {
            if (staging != null) {
                deleteTree(staging);
            }
        }
    }

    /** Removes an installed driver, and the choice with it when that driver was chosen. */
    static void delete(File filesDir, String id) {
        if (!plainName(id)) {
            return;
        }
        boolean automatic = AUTOMATIC.equals(mode(filesDir));
        if (id.equals(selected(filesDir))) {
            select(filesDir, null);
        }
        deleteTree(new File(root(filesDir), id));
        // Automatic now resolves to something else: another installed driver, or the system one.
        if (automatic) {
            select(filesDir, AUTOMATIC);
        }
    }

    // ---- the files --------------------------------------------------------------------

    /**
     * The driver library in {@code folder}: the one meta.json names if it is there, else the
     * folder's single .so. Null when there is none, more than one, or the file is not an
     * AArch64 shared object, each of which would only fail later inside the game.
     */
    static String libraryIn(File folder) {
        String named = metaField(read(new File(folder, META)), "libraryName");
        if (named != null && plainName(named)) {
            File library = new File(folder, named);
            return library.isFile() && isAarch64Elf(library) ? named : null;
        }
        String[] names = folder.list();
        if (names == null) {
            return null;
        }
        String only = null;
        for (String name : names) {
            if (!name.endsWith(".so")) {
                continue;
            }
            File library = new File(folder, name);
            if (!library.isFile() || !isAarch64Elf(library)) {
                continue;
            }
            if (only != null) {
                return null; // Two drivers in one folder: meta.json must say.
            }
            only = name;
        }
        return only;
    }

    /** What to call the driver: meta.json's name and version, else the folder's name. */
    static String label(File folder) {
        String text = read(new File(folder, META));
        String name = metaField(text, "name");
        String version = metaField(text, "driverVersion");
        if (name == null) {
            return folder.getName();
        }
        return version == null ? name : name + " " + version;
    }

    static boolean isAarch64Elf(File file) {
        byte[] head = new byte[20];
        try (InputStream in = new FileInputStream(file)) {
            int read = 0;
            while (read < head.length) {
                int n = in.read(head, read, head.length - read);
                if (n < 0) {
                    return false;
                }
                read += n;
            }
        } catch (IOException e) {
            return false;
        }
        if (!Arrays.equals(Arrays.copyOf(head, ELF64_LE.length), ELF64_LE)) {
            return false;
        }
        // e_machine, two bytes at 0x12, little-endian.
        return ((head[0x12] & 0xff) | ((head[0x13] & 0xff) << 8)) == EM_AARCH64;
    }

    private static void unpack(InputStream source, File into) throws IOException {
        try (ZipInputStream in = new ZipInputStream(source)) {
            long total = 0;
            int entries = 0;
            ZipEntry entry;
            while ((entry = in.getNextEntry()) != null) {
                if (entry.isDirectory()) {
                    continue;
                }
                if (++entries > MAX_ENTRIES) {
                    throw new IOException("That zip holds too many files for a driver");
                }
                // Only the file's own name is used, never the path inside the zip, so no
                // entry can escape this folder however it is named.
                String name = new File(entry.getName().replace('\\', '/')).getName();
                if (!plainName(name)) {
                    continue;
                }
                try (OutputStream sink = new FileOutputStream(new File(into, name))) {
                    byte[] buffer = new byte[64 * 1024];
                    int n;
                    while ((n = in.read(buffer)) > 0) {
                        total += n;
                        if (total > MAX_UNPACKED_BYTES) {
                            throw new IOException("That zip is far too large for a driver");
                        }
                        sink.write(buffer, 0, n);
                    }
                }
            }
            if (entries == 0) {
                throw new IOException("That zip is empty");
            }
        }
    }

    /** A folder name for a driver labelled {@code label} that nothing else is using yet. */
    static String freeId(File root, String label) {
        StringBuilder id = new StringBuilder();
        for (char c : label.toLowerCase(Locale.ROOT).toCharArray()) {
            boolean keep = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '.' || c == '-';
            id.append(keep ? c : '-');
        }
        // A leading dash is noise, and a leading dot would hide the folder from
        // {@link #installed} and could make the name a climb out of the folder.
        while (id.length() > 0 && (id.charAt(0) == '-' || id.charAt(0) == '.')) {
            id.deleteCharAt(0);
        }
        while (id.length() > 40) {
            id.deleteCharAt(id.length() - 1);
        }
        String base = plainName(id.toString()) ? id.toString() : "driver";
        if (base.equals(AUTOMATIC)) {
            // Automatic is the launcher's own word for a row, never a folder.
            base = base + "-driver";
        }
        String candidate = base;
        for (int n = 2; new File(root, candidate).exists(); n++) {
            candidate = base + "-" + n;
        }
        return candidate;
    }

    private static void deleteTree(File file) {
        File[] children = file.listFiles();
        if (children != null) {
            for (File child : children) {
                deleteTree(child);
            }
        }
        file.delete();
    }

    // ---- small readers and writers ----------------------------------------------------

    /** A text file's contents, or null. Bounded: these files are a few hundred bytes. */
    private static String read(File file) {
        try {
            if (!file.isFile() || file.length() > 64 * 1024) {
                return null;
            }
            return new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
        } catch (IOException | RuntimeException e) {
            return null;
        }
    }

    /** Writes {@code text} in one step, so a reader never sees half a file. */
    private static void write(File file, String text) {
        File temp = new File(file.getPath() + ".tmp");
        try {
            try (OutputStream out = new FileOutputStream(temp)) {
                out.write(text.getBytes(StandardCharsets.UTF_8));
            }
            Files.move(temp.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING);
        } catch (IOException | RuntimeException e) {
            temp.delete();
        }
    }

    /** A {@code name value} line's value, as the game's own option files are written. */
    static String value(String text, String name) {
        if (text == null) {
            return null;
        }
        for (String line : text.split("\n")) {
            String trimmed = line.trim();
            if (!trimmed.startsWith(name + " ")) {
                continue;
            }
            String found = trimmed.substring(name.length() + 1).trim();
            if (!found.isEmpty()) {
                return found;
            }
        }
        return null;
    }

    /**
     * One string field of a driver's meta.json (the emulator driver format: {@code name},
     * {@code driverVersion}, {@code libraryName}), or null when it is absent or empty.
     *
     * <p>Read here rather than with {@code org.json}, so this whole class is plain Java and
     * can be tested without an Android platform. Only a string value is understood, which is
     * all three of those fields are; a {@code \\u} escape is taken as the letters it is made
     * of, because a driver's name is not where one belongs.
     */
    static String metaField(String json, String field) {
        if (json == null) {
            return null;
        }
        String key = "\"" + field + "\"";
        for (int at = json.indexOf(key); at >= 0; at = json.indexOf(key, at + 1)) {
            int i = at + key.length();
            while (i < json.length() && Character.isWhitespace(json.charAt(i))) {
                i++;
            }
            if (i >= json.length() || json.charAt(i) != ':') {
                continue;
            }
            i++;
            while (i < json.length() && Character.isWhitespace(json.charAt(i))) {
                i++;
            }
            if (i >= json.length() || json.charAt(i) != '"') {
                continue;
            }
            StringBuilder found = new StringBuilder();
            for (i++; i < json.length(); i++) {
                char c = json.charAt(i);
                if (c == '\\') {
                    if (++i >= json.length()) {
                        return null;
                    }
                    char escaped = json.charAt(i);
                    found.append(escaped == 'n' ? '\n' : escaped == 't' ? '\t' : escaped);
                    continue;
                }
                if (c == '"') {
                    String text = found.toString().trim();
                    return text.isEmpty() ? null : text;
                }
                found.append(c);
            }
            return null;
        }
        return null;
    }

    /** One path element: never a path, never a climb out of the folder it is joined to. */
    static boolean plainName(String name) {
        return name != null && !name.isEmpty() && !name.equals(".") && !name.equals("..")
            && name.indexOf('/') < 0 && name.indexOf('\\') < 0 && name.indexOf('\0') < 0;
    }

    /** Keeps a label on the one line the game's option files give it. */
    private static String oneLine(String text) {
        return text.replace('\r', ' ').replace('\n', ' ').trim();
    }
}
