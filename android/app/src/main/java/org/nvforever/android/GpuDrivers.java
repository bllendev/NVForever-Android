package org.nvforever.android;

import android.content.Context;
import android.net.Uri;

import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * The Vulkan drivers the player installed themselves, and which of them the next game start
 * uses. NVForever ships no driver: the player supplies a driver zip in the emulator format (a
 * single .so, optionally with meta.json naming it), and the game opens it through libadrenotools
 * (engine/nvf-android/src/gpu_driver.rs, docs/odin-debug.md).
 *
 * Everything lives in the app's private storage, which is what libadrenotools requires: a
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
 * word: it resolves, every time it is chosen, to an installed driver if there is one and to
 * the system driver if there is not. NVForever bundles no driver, so on a fresh install
 * Automatic is exactly the system driver.
 *
 * With nothing installed and nothing selected the game behaves exactly as it did before this
 * existed: no file is there, so none is read.
 */
public final class GpuDrivers {
    /**
     * The sentinel {@link #select} and {@link #mode} use for Automatic: an installed driver
     * when there is one, the system driver when there is not. Never a folder name: the ids
     * {@link #install} makes are derived from a driver's own label.
     */
    public static final String AUTOMATIC = "automatic";

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

    private GpuDrivers() {}

    // ---- what is installed ------------------------------------------------------------

    /** The installed drivers as {id, label}, by label. Empty when none is installed. */
    public static List<String[]> installed(Context context) {
        List<String[]> drivers = new ArrayList<>();
        File[] folders = root(context).listFiles();
        if (folders == null) return drivers;
        for (File folder : folders) {
            if (!folder.isDirectory() || folder.getName().startsWith(".")) continue;
            String library = libraryIn(folder);
            if (library == null) continue;
            drivers.add(new String[] { folder.getName(), label(folder) });
        }
        Collections.sort(drivers, (a, b) -> a[1].compareToIgnoreCase(b[1]));
        return drivers;
    }

    /** The chosen driver's id, or null for the system driver (including a stale choice). */
    public static String selected(Context context) {
        String folder = value(read(new File(root(context), SELECTED)), "folder");
        if (folder == null || !plainName(folder)) return null;
        File dir = new File(root(context), folder);
        return dir.isDirectory() && libraryIn(dir) != null ? folder : null;
    }

    /**
     * Which row of the Graphics driver choice is filled in: {@link #AUTOMATIC}, null for the
     * system driver, or an installed driver's id. This is the player's own choice, which is
     * not always the folder the next start opens: Automatic with nothing installed is the
     * system driver, and the row still reads Automatic.
     */
    public static String mode(Context context) {
        String stored = read(new File(root(context), MODE));
        if (stored != null && AUTOMATIC.equals(stored.trim())) return AUTOMATIC;
        return selected(context);
    }

    /**
     * Chooses a driver for the next game start: {@link #AUTOMATIC}, a driver's id, or null for
     * the system driver (which is also what an id naming nothing installed means). Either way
     * the previous start's marker and fallback note go: the question they answer was about the
     * choice being replaced.
     */
    public static void select(Context context, String idOrNull) {
        if (AUTOMATIC.equals(idOrNull)) {
            List<String[]> drivers = installed(context);
            apply(context, drivers.isEmpty() ? null : drivers.get(0)[0]);
            // Written after the choice, so a crash in between leaves the plainer of the two
            // answers: whatever `selected` says, read as a direct choice.
            write(new File(root(context), MODE), AUTOMATIC + "\n");
            return;
        }
        //noinspection ResultOfMethodCallIgnored
        new File(root(context), MODE).delete();
        apply(context, idOrNull);
    }

    /** Writes the folder the next start opens, or removes the file for the system driver. */
    private static void apply(Context context, String idOrNull) {
        File root = root(context);
        File chosen = idOrNull != null && plainName(idOrNull) ? new File(root, idOrNull) : null;
        String library = chosen != null && chosen.isDirectory() ? libraryIn(chosen) : null;
        //noinspection ResultOfMethodCallIgnored
        root.mkdirs();
        new File(root, MARKER).delete();
        new File(root, LAST_FALLBACK).delete();
        if (library == null) {
            new File(root, SELECTED).delete();
            return;
        }
        // The game asks the linker where its own libraries were extracted to and only reads
        // this when that fails, but it costs nothing to leave the answer here.
        write(new File(root, NATIVE_LIB_DIR), context.getApplicationInfo().nativeLibraryDir + "\n");
        write(new File(root, SELECTED), "# Written by the launcher; applied when starting the game\n"
                + "folder " + idOrNull + "\n"
                + "library " + library + "\n"
                + "label " + oneLine(label(chosen)) + "\n");
    }

    /** Why the last game start did not use the chosen driver, or null when it did. */
    public static String lastStartFellBack(Context context) {
        String why = read(new File(root(context), LAST_FALLBACK));
        if (why == null) return null;
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
    public static String install(Context context, Uri zip) throws IOException {
        File root = root(context);
        if (!root.isDirectory() && !root.mkdirs()) throw new IOException("Cannot create " + root);
        File staging = new File(root, ".incoming-" + System.nanoTime());
        if (!staging.mkdirs()) throw new IOException("Cannot create a folder for the driver");
        try {
            unpack(context, zip, staging);
            String library = libraryIn(staging);
            if (library == null) {
                throw new IOException("That zip holds no 64-bit ARM Vulkan driver (one .so is expected)");
            }
            String id = freeId(root, label(staging));
            File target = new File(root, id);
            if (!staging.renameTo(target)) throw new IOException("Cannot store the driver as " + id);
            staging = null;
            // Automatic means "an installed driver if there is one", so a first install is
            // what it has been waiting for. A direct choice is left exactly as the player made it.
            if (AUTOMATIC.equals(mode(context))) select(context, AUTOMATIC);
            return id;
        } finally {
            if (staging != null) deleteTree(staging);
        }
    }

    /** Removes an installed driver, and the choice with it when that driver was chosen. */
    public static void delete(Context context, String id) {
        if (!plainName(id)) return;
        boolean automatic = AUTOMATIC.equals(mode(context));
        if (id.equals(selected(context))) select(context, null);
        deleteTree(new File(root(context), id));
        // Automatic now resolves to something else: another installed driver, or the system one.
        if (automatic) select(context, AUTOMATIC);
    }

    // ---- the files --------------------------------------------------------------------

    private static File root(Context context) {
        return new File(context.getFilesDir(), "gpu_drivers");
    }

    /**
     * The driver library in {@code folder}: the one meta.json names if it is there, else the
     * folder's single .so. Null when there is none, more than one, or the file is not an
     * AArch64 shared object, each of which would only fail later inside the game.
     */
    private static String libraryIn(File folder) {
        String named = meta(read(new File(folder, META)), "libraryName");
        if (named != null && plainName(named)) {
            File library = new File(folder, named);
            return library.isFile() && isAarch64Elf(library) ? named : null;
        }
        String[] names = folder.list();
        if (names == null) return null;
        String only = null;
        for (String name : names) {
            if (!name.endsWith(".so")) continue;
            File library = new File(folder, name);
            if (!library.isFile() || !isAarch64Elf(library)) continue;
            if (only != null) return null; // Two drivers in one folder: meta.json must say.
            only = name;
        }
        return only;
    }

    /** What to call the driver: meta.json's name and version, else the folder's name. */
    private static String label(File folder) {
        String text = read(new File(folder, META));
        String name = meta(text, "name");
        String version = meta(text, "driverVersion");
        if (name == null || name.trim().isEmpty()) return folder.getName();
        return version == null || version.trim().isEmpty() ? name.trim() : name.trim() + " " + version.trim();
    }

    private static boolean isAarch64Elf(File file) {
        byte[] head = new byte[20];
        try (InputStream in = new java.io.FileInputStream(file)) {
            int read = 0;
            while (read < head.length) {
                int n = in.read(head, read, head.length - read);
                if (n < 0) return false;
                read += n;
            }
        } catch (IOException e) {
            return false;
        }
        if (!Arrays.equals(Arrays.copyOf(head, ELF64_LE.length), ELF64_LE)) return false;
        // e_machine, two bytes at 0x12, little-endian.
        return ((head[0x12] & 0xff) | ((head[0x13] & 0xff) << 8)) == EM_AARCH64;
    }

    private static void unpack(Context context, Uri zip, File into) throws IOException {
        try (InputStream source = context.getContentResolver().openInputStream(zip)) {
            if (source == null) throw new IOException("The zip could not be opened");
            try (ZipInputStream in = new ZipInputStream(source)) {
                long total = 0;
                int entries = 0;
                ZipEntry entry;
                while ((entry = in.getNextEntry()) != null) {
                    if (entry.isDirectory()) continue;
                    if (++entries > MAX_ENTRIES) throw new IOException("That zip holds too many files for a driver");
                    // Only the file's own name is used, never the path inside the zip, so no
                    // entry can escape this folder however it is named.
                    String name = new File(entry.getName().replace('\\', '/')).getName();
                    if (!plainName(name)) continue;
                    File out = new File(into, name);
                    try (OutputStream sink = new FileOutputStream(out)) {
                        byte[] buffer = new byte[64 * 1024];
                        int n;
                        while ((n = in.read(buffer)) > 0) {
                            total += n;
                            if (total > MAX_UNPACKED_BYTES) throw new IOException("That zip is far too large for a driver");
                            sink.write(buffer, 0, n);
                        }
                    }
                }
                if (entries == 0) throw new IOException("That zip is empty");
            }
        }
    }

    /** A folder name for a driver labelled {@code label} that nothing else is using yet. */
    private static String freeId(File root, String label) {
        StringBuilder id = new StringBuilder();
        for (char c : label.toLowerCase(Locale.ROOT).toCharArray()) {
            boolean keep = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '.' || c == '-';
            id.append(keep ? c : '-');
        }
        while (id.length() > 0 && id.charAt(0) == '-') id.deleteCharAt(0);
        while (id.length() > 40) id.deleteCharAt(id.length() - 1);
        String base = id.length() == 0 ? "driver" : id.toString();
        String candidate = base;
        for (int n = 2; new File(root, candidate).exists(); n++) candidate = base + "-" + n;
        return candidate;
    }

    private static void deleteTree(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) deleteTree(child);
        //noinspection ResultOfMethodCallIgnored
        file.delete();
    }

    // ---- small readers and writers ----------------------------------------------------

    /** A text file's contents, or null. Bounded: these files are a few hundred bytes. */
    private static String read(File file) {
        try {
            if (!file.isFile() || file.length() > 64 * 1024) return null;
            return new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
        } catch (IOException | SecurityException e) {
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
        } catch (IOException e) {
            android.util.Log.e("NVForever", "Writing " + file.getName(), e);
            //noinspection ResultOfMethodCallIgnored
            temp.delete();
        }
    }

    /** A {@code name value} line's value, as the game's own option files are written. */
    private static String value(String text, String name) {
        if (text == null) return null;
        for (String line : text.split("\n")) {
            String trimmed = line.trim();
            if (!trimmed.startsWith(name + " ")) continue;
            String value = trimmed.substring(name.length() + 1).trim();
            if (!value.isEmpty()) return value;
        }
        return null;
    }

    /** A meta.json field (the emulator driver format: name, driverVersion, libraryName). */
    private static String meta(String json, String field) {
        if (json == null) return null;
        try {
            String value = new JSONObject(json).optString(field, "");
            return value.isEmpty() ? null : value;
        } catch (org.json.JSONException e) {
            return null;
        }
    }

    /** One path element: never a path, never a climb out of the folder it is joined to. */
    private static boolean plainName(String name) {
        return name != null && !name.isEmpty() && !name.equals(".") && !name.equals("..")
                && name.indexOf('/') < 0 && name.indexOf('\\') < 0 && name.indexOf('\0') < 0;
    }

    /** Keeps a label on the one line the game's option files give it. */
    private static String oneLine(String text) {
        return text.replace('\r', ' ').replace('\n', ' ').trim();
    }
}
