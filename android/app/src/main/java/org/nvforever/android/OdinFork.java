package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * The Android fork's own decisions, independent of Android APIs so they can be tested as
 * plain Java (docs/fork.md). Upstream's classes call into this rather than carrying the
 * fork's values themselves, which keeps an upstream merge to a call site at a time.
 *
 * <p>Two things live here:
 *
 * <ul>
 * <li>{@link #HOME} is the engine home folder's name inside the app's private files
 * directory. Upstream calls it {@code home}; the fork keeps {@code NVForever}, the name every
 * install of this fork already has, so an upgrade finds the player's saves, NVForever.ini,
 * plugins.txt and logs exactly where they are. The engine agrees through
 * {@code engine/nvforever/src/odin/home.rs}.</li>
 * <li>{@code data-path.txt} names a game folder the fork reads in place from shared storage,
 * instead of importing a copy into app storage. One line, written by the fork's launcher
 * (and by every earlier build of it). When it is absent, upstream's importer is what runs.</li>
 * </ul>
 */
final class OdinFork {
    /** The engine home inside the app's private files directory. */
    static final String HOME = "NVForever";

    /** The file naming a game folder to read in place, one line. */
    static final String DATA_PATH = "data-path.txt";

    /** The default folder the fork's builds look in when nothing was chosen. */
    static final String SHARED_DATA = "/storage/emulated/0/NVForever/Data";

    private OdinFork() {
    }

    /** The engine home, which upstream would place at {@code files/home}. */
    static File home(File filesDir) {
        return new File(filesDir, HOME);
    }

    /** Where {@link #DATA_PATH} lives: beside the home, in the private files directory. */
    static File dataPathFile(File filesDir) {
        return new File(filesDir, DATA_PATH);
    }

    /**
     * The game folder named by {@code data-path.txt}: its first line with the surrounding
     * space removed, or null when the file is absent, unreadable, empty or not one path.
     * Only an absolute path is accepted, because a relative one would mean a different
     * folder in every process.
     */
    static String dataPath(File filesDir) {
        File file = dataPathFile(filesDir);
        if (!file.isFile() || file.length() > 4096) return null;
        String text;
        try {
            text = new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
        } catch (IOException | RuntimeException e) {
            return null;
        }
        for (String line : text.split("\\R")) {
            String trimmed = line.trim();
            if (trimmed.isEmpty() || trimmed.startsWith("#")) continue;
            return trimmed.startsWith("/") && trimmed.indexOf('\0') < 0 ? trimmed : null;
        }
        return null;
    }

    /**
     * Whether this install reads its game folder in place, which is what decides whether the
     * launcher has to ask for all files access. An install with no {@code data-path.txt} is
     * upstream's: it imports a copy and needs no storage permission.
     */
    static boolean readsInPlace(File filesDir) {
        return dataPath(filesDir) != null;
    }
    /**
     * Whether the launcher's game folder is read where it is rather than a copy the importer
     * made: imports live under the app's external files directory, anything else is in place.
     */
    static boolean inPlaceFolder(android.content.Context context, String folder) {
        if (folder == null) return false;
        File imports = context.getExternalFilesDir(null);
        return imports == null || !folder.startsWith(imports.getAbsolutePath());
    }
}
