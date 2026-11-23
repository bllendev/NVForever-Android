package org.nvforever.android;

import java.io.File;
import java.util.Locale;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * What the active profile's folders say without opening a save or parsing a log: the newest
 * save, and whether the last run ended normally.
 *
 * <p>Nothing here reads a save's contents. Home shows a file name, a time and a folder, and
 * that is all it is allowed to know.
 *
 * <p>Plain Java on purpose, so it is tested with javac and java alone.
 */
public final class RunState {
    /** {@code nvforever-20261120-120000-42.log}: the stamp and the process id. */
    private static final Pattern RUN = Pattern.compile("nvforever-(\\d{8}-\\d{6})-(\\d+)\\.log");
    /** {@code crash-20261120-120001-42.txt}: the engine's own crash report for that run. */
    private static final Pattern CRASH = Pattern.compile("crash-(\\d{8}-\\d{6})-(\\d+)\\.txt");

    private RunState() {}

    /** The newest save of a profile: its name without the extension, and when it was written. */
    public static final class Save {
        public final File file;
        public final long modified;

        Save(File file, long modified) {
            this.file = file;
            this.modified = modified;
        }

        /** The save's name as the player named it, without {@code .nvfsave}. */
        public String name() {
            String n = file.getName();
            int dot = n.lastIndexOf('.');
            return dot > 0 ? n.substring(0, dot) : n;
        }
    }

    /** The newest {@code .nvfsave} in a profile's saves folder, or null when there is none. */
    public static Save newestSave(File savesDir) {
        File[] files = savesDir.listFiles();
        if (files == null) {
            return null;
        }
        File newest = null;
        for (File file : files) {
            if (!file.isFile() || !file.getName().toLowerCase(Locale.ROOT).endsWith(".nvfsave")) {
                continue;
            }
            if (newest == null || file.lastModified() > newest.lastModified()) {
                newest = file;
            }
        }
        return newest == null ? null : new Save(newest, newest.lastModified());
    }

    /** How many saves a profile has. */
    public static int countSaves(File savesDir) {
        File[] files = savesDir.listFiles();
        if (files == null) {
            return 0;
        }
        int n = 0;
        for (File file : files) {
            if (file.isFile() && file.getName().toLowerCase(Locale.ROOT).endsWith(".nvfsave")) {
                n++;
            }
        }
        return n;
    }

    /** How the last run ended. */
    public enum LastRun {
        /** The profile has no run log yet. */
        NONE,
        /** A run log, and no crash report belonging to it. */
        OK,
        /** The engine wrote a crash report for the newest run. */
        CRASHED,
    }

    /**
     * Reads the newest run log's name in a profile's logs folder and looks for a crash report
     * of the same process id, written at or after the run started: exactly the pairing
     * {@link DebugReport} exports. No log is opened, so this is cheap enough for every resume.
     */
    public static LastRun lastRun(File logsDir) {
        File[] files = logsDir.listFiles();
        if (files == null) {
            return LastRun.NONE;
        }
        String stamp = null;
        String pid = null;
        for (File file : files) {
            Matcher m = RUN.matcher(file.getName());
            if (file.isFile() && m.matches() && (stamp == null || m.group(1).compareTo(stamp) > 0)) {
                stamp = m.group(1);
                pid = m.group(2);
            }
        }
        if (stamp == null) {
            return LastRun.NONE;
        }
        for (File file : files) {
            Matcher m = CRASH.matcher(file.getName());
            if (file.isFile() && m.matches() && m.group(2).equals(pid) && m.group(1).compareTo(stamp) >= 0) {
                return LastRun.CRASHED;
            }
        }
        return LastRun.OK;
    }
}
