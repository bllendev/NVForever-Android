package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;

/**
 * Backing a profile's saves up to the phone's Download folder, and reading them back.
 *
 * <pre>
 * Download/NVForever-backups/&lt;profile&gt;-&lt;date&gt;/   one backup: a plain folder of .nvfsave files
 * </pre>
 *
 * <p>Two rules this class exists to keep, and the tests check both:
 *
 * <ul>
 *   <li><b>A copy, never a move.</b> Nothing here deletes anything. Backing up copies out of
 *       the profile's saves folder and leaves it exactly as it was; a name already in the
 *       backup is skipped rather than replaced.
 *   <li><b>Nothing is lost on a restore.</b> A save the restore would write over is copied to
 *       {@code <name>.bak} first, and if that name is taken too, to {@code .bak-2} and so on,
 *       so no earlier file and no earlier backup of one is ever overwritten.
 * </ul>
 *
 * <p>It never touches a Data folder, a plugin list or a log, and it never writes anywhere but
 * the backup folder named here and the profile's own saves folder.
 *
 * <p>Plain Java on purpose, so it is tested with javac and java alone.
 */
public final class SaveBackup {
    /** The folder in Download that holds every backup, one subfolder each. */
    public static final String BACKUPS_TAIL = "NVForever-backups";

    private static final String SAVE_SUFFIX = ".nvfsave";
    private static final DateTimeFormatter STAMP =
            DateTimeFormatter.ofPattern("yyyyMMdd-HHmm", Locale.ROOT);

    private SaveBackup() {}

    /** {@code Download/NVForever-backups}. */
    public static File backupsRoot(File downloads) {
        return new File(downloads, BACKUPS_TAIL);
    }

    /** The folder one backup of {@code profile} taken at {@code when} is written to. */
    public static File folderFor(File downloads, String profile, long when) {
        String stamp = STAMP.format(Instant.ofEpochMilli(when).atZone(ZoneId.systemDefault()));
        return new File(backupsRoot(downloads), safe(profile) + "-" + stamp);
    }

    /** How many saves a folder holds. */
    public static int countSaves(File dir) {
        return saves(dir).size();
    }

    /** The {@code .nvfsave} files in a folder, by name. Empty when it cannot be listed. */
    public static List<File> saves(File dir) {
        List<File> found = new ArrayList<>();
        File[] files = dir.listFiles();
        if (files == null) {
            return found;
        }
        for (File file : files) {
            if (file.isFile() && file.getName().toLowerCase(Locale.ROOT).endsWith(SAVE_SUFFIX)) {
                found.add(file);
            }
        }
        Collections.sort(found, (a, b) -> a.getName().compareToIgnoreCase(b.getName()));
        return found;
    }

    /**
     * Copies every save of {@code savesDir} into {@code into}, which is created if it is not
     * there. Returns how many were copied. A name already in {@code into} is left alone, so
     * running a backup twice into one folder adds what is new and replaces nothing.
     *
     * @throws IOException if the folder cannot be made or a file cannot be copied
     */
    public static int backUp(File savesDir, File into) throws IOException {
        List<File> saves = saves(savesDir);
        if (saves.isEmpty()) {
            return 0;
        }
        if (!into.isDirectory() && !into.mkdirs()) {
            throw new IOException("Cannot create " + into);
        }
        int copied = 0;
        for (File save : saves) {
            File target = new File(into, save.getName());
            if (target.exists()) {
                continue;
            }
            Files.copy(save.toPath(), target.toPath(), StandardCopyOption.COPY_ATTRIBUTES);
            copied++;
        }
        return copied;
    }

    /** Every backup folder under Download, newest first. Empty when there is none. */
    public static List<File> backups(File downloads) {
        List<File> found = new ArrayList<>();
        File[] folders = backupsRoot(downloads).listFiles();
        if (folders == null) {
            return found;
        }
        for (File folder : folders) {
            if (folder.isDirectory() && !folder.getName().startsWith(".") && countSaves(folder) > 0) {
                found.add(folder);
            }
        }
        Collections.sort(found, (a, b) -> {
            int byTime = Long.compare(b.lastModified(), a.lastModified());
            return byTime != 0 ? byTime : b.getName().compareToIgnoreCase(a.getName());
        });
        return found;
    }

    /** How many of a backup's saves already exist in {@code savesDir} under the same name. */
    public static int clashes(File backup, File savesDir) {
        int n = 0;
        for (File save : saves(backup)) {
            if (new File(savesDir, save.getName()).exists()) {
                n++;
            }
        }
        return n;
    }

    /**
     * Copies a backup's saves into {@code savesDir}, returning how many were written. A save
     * already there is copied to a free {@code .bak} name first and only then written over, so
     * the restore can always be undone by hand and no file is lost.
     *
     * @throws IOException if the folder cannot be made or a file cannot be copied
     */
    public static int restore(File backup, File savesDir) throws IOException {
        List<File> saves = saves(backup);
        if (saves.isEmpty()) {
            return 0;
        }
        if (!savesDir.isDirectory() && !savesDir.mkdirs()) {
            throw new IOException("Cannot create " + savesDir);
        }
        int restored = 0;
        for (File save : saves) {
            File target = new File(savesDir, save.getName());
            if (target.exists()) {
                Files.copy(target.toPath(), freeBackupName(target).toPath(),
                        StandardCopyOption.COPY_ATTRIBUTES);
            }
            Files.copy(save.toPath(), target.toPath(),
                    StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.COPY_ATTRIBUTES);
            restored++;
        }
        return restored;
    }

    /** {@code save.nvfsave.bak}, or {@code .bak-2} and up when earlier ones are there. */
    static File freeBackupName(File file) {
        File candidate = new File(file.getPath() + ".bak");
        for (int n = 2; candidate.exists(); n++) {
            candidate = new File(file.getPath() + ".bak-" + n);
        }
        return candidate;
    }

    /** A profile's name turned into one folder name: no path, no separator, never empty. */
    static String safe(String profile) {
        StringBuilder sb = new StringBuilder();
        boolean dash = false;
        for (char c : profile.trim().toLowerCase(Locale.ROOT).toCharArray()) {
            if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9')) {
                sb.append(c);
                dash = false;
            } else if (sb.length() > 0 && !dash) {
                sb.append('-');
                dash = true;
            }
        }
        while (sb.length() > 0 && sb.charAt(sb.length() - 1) == '-') {
            sb.setLength(sb.length() - 1);
        }
        if (sb.length() > 32) {
            sb.setLength(32);
        }
        return sb.length() == 0 ? "profile" : sb.toString();
    }
}
