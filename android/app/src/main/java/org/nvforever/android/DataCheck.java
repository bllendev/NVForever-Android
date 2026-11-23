package org.nvforever.android;

import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Does a folder hold a Fallout: New Vegas Data the engine will accept?
 *
 * <p>The two lists are the engine's own, copied from {@code fnv_install::REQUIRED_DATA_FILES}
 * and {@code fnv_install::ADDON_MASTERS}, so the launcher can never pass a folder the engine
 * then refuses. A file of zero length counts as missing, the way
 * {@code GameInstall::missing_files} treats it: that is what an interrupted copy leaves.
 *
 * <p>Plain Java on purpose (no Android classes), so it is tested with javac and java alone.
 */
public final class DataCheck {
    /** What the engine cannot run without: the base master and seven archives. */
    public static final String[] REQUIRED_DATA_FILES = {
        "FalloutNV.esm",
        "Fallout - Textures.bsa",
        "Fallout - Textures2.bsa",
        "Fallout - Meshes.bsa",
        "Fallout - Voices1.bsa",
        "Fallout - Sound.bsa",
        "Fallout - Misc.bsa",
        "Update.bsa",
    };

    /** The add-on masters the engine loads when they are there. All of them are optional. */
    public static final String[] ADDON_MASTERS = {
        "DeadMoney.esm",
        "HonestHearts.esm",
        "OldWorldBlues.esm",
        "LonesomeRoad.esm",
        "GunRunnersArsenal.esm",
        "ClassicPack.esm",
        "MercenaryPack.esm",
        "TribalPack.esm",
        "CaravanPack.esm",
    };

    /** One checked file: its name, whether it is usable, and its size in bytes. */
    public static final class Entry {
        public final String name;
        public final boolean present;
        public final long size;

        Entry(String name, boolean present, long size) {
            this.name = name;
            this.present = present;
            this.size = size;
        }
    }

    /** What a scan found. */
    public static final class Result {
        /** The folder the scan looked at. */
        public final File folder;
        /** The folder is under Android/data or Android/obb, which Android closes to every app. */
        public final boolean closed;
        /** The folder exists and can be listed. */
        public final boolean exists;
        public final List<Entry> required;
        public final List<Entry> addons;

        Result(File folder, boolean closed, boolean exists, List<Entry> required, List<Entry> addons) {
            this.folder = folder;
            this.closed = closed;
            this.exists = exists;
            this.required = required;
            this.addons = addons;
        }

        /** The game can start on this folder. */
        public boolean ready() {
            return exists && !closed && missing().isEmpty();
        }

        /** The required files that are absent or empty, in the engine's own order. */
        public List<String> missing() {
            List<String> out = new ArrayList<>();
            for (Entry e : required) {
                if (!e.present) {
                    out.add(e.name);
                }
            }
            return out;
        }

        public int requiredFound() {
            return required.size() - missing().size();
        }

        public int archivesFound() {
            int n = 0;
            for (Entry e : required) {
                if (e.present && !e.name.toLowerCase(Locale.ROOT).endsWith(".esm")) {
                    n++;
                }
            }
            return n;
        }

        public int archivesTotal() {
            int n = 0;
            for (Entry e : required) {
                if (!e.name.toLowerCase(Locale.ROOT).endsWith(".esm")) {
                    n++;
                }
            }
            return n;
        }

        public boolean baseMasterFound() {
            return required.get(0).present;
        }

        public int addonsFound() {
            int n = 0;
            for (Entry e : addons) {
                if (e.present) {
                    n++;
                }
            }
            return n;
        }
    }

    private DataCheck() {}

    /** Scans {@code dir} once and judges it. Never throws. */
    public static Result scan(File dir) {
        boolean closed = isClosedFolder(dir);
        String[] names = dir.list();
        boolean exists = names != null;
        List<Entry> required = new ArrayList<>();
        List<Entry> addons = new ArrayList<>();
        for (String want : REQUIRED_DATA_FILES) {
            required.add(entry(dir, names, want));
        }
        for (String want : ADDON_MASTERS) {
            addons.add(entry(dir, names, want));
        }
        return new Result(dir, closed, exists, required, addons);
    }

    private static Entry entry(File dir, String[] names, String want) {
        File found = find(dir, names, want);
        long size = found == null ? 0L : found.length();
        return new Entry(want, found != null && size > 0L, size);
    }

    /**
     * The file in {@code names} whose name is {@code want}, whatever its case. The game's own
     * files are copied from a Windows install, where case never mattered, so a folder holding
     * {@code falloutnv.esm} is the same folder.
     */
    private static File find(File dir, String[] names, String want) {
        if (names == null) {
            return null;
        }
        for (String name : names) {
            if (name.equalsIgnoreCase(want)) {
                return new File(dir, name);
            }
        }
        return null;
    }

    /**
     * Android closes the folders under {@code Android/data} and {@code Android/obb} to every
     * app since Android 11, with or without all-files access, so no Data folder can live there.
     */
    public static boolean isClosedFolder(File dir) {
        String p = dir.getPath().replace('\\', '/');
        return p.contains("/Android/data/") || p.endsWith("/Android/data")
                || p.contains("/Android/obb/") || p.endsWith("/Android/obb");
    }

    /** If the player picked the game's root folder, the {@code Data} folder inside it is meant. */
    public static File normalize(File picked) {
        if (find(picked, picked.list(), "FalloutNV.esm") != null) {
            return picked;
        }
        File inner = new File(picked, "Data");
        if (find(inner, inner.list(), "FalloutNV.esm") != null) {
            return inner;
        }
        return picked;
    }
}
