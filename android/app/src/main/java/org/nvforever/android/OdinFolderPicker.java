package org.nvforever.android;

import android.app.Activity;
import android.app.AlertDialog;
import java.io.File;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/**
 * An in-app folder browser over {@code /storage}, for choosing the game folder to read in
 * place (docs/fork.md).
 *
 * <p>Upstream picks the game folder with {@code ACTION_OPEN_DOCUMENT_TREE}, which yields a
 * Storage Access Framework tree URI. That is the right thing for an importer, which reads the
 * tree through a content provider and copies it. It is no use at all for reading in place: a
 * tree URI is not a filesystem path, the engine opens the game's archives with ordinary file
 * reads, and there is no supported way to turn one into the other.
 *
 * <p>This fork's builds hold all files access ({@link OdinStorage}), so a path is all that is
 * needed, and a plain list of folders is also the only thing on this page a controller can
 * work: an AlertDialog's list takes the D-pad and the A button with no extra code, where the
 * system picker is a touch-first app of its own.
 *
 * <p>Nothing here opens, reads or writes a single game file: it lists directory names and
 * hands one path back. {@code crate::odin::choose_folder} is what checks that the folder is a
 * complete install, and it does that before anything is remembered.
 */
final class OdinFolderPicker {
    /** Where browsing starts: every storage volume on the device is under here. */
    static final File STORAGE = new File("/storage");

    /** How many names one page of the list may show, so a huge folder cannot stall the UI. */
    private static final int MAX_ENTRIES = 400;

    interface Chosen {
        void folder(File dir);
    }

    private OdinFolderPicker() {
    }

    /**
     * Shows the list for {@code at}, re-showing itself as the player moves through folders.
     * {@code chosen} is called with the folder they settle on, and not called at all if they
     * leave.
     */
    static void show(Activity activity, File at, Chosen chosen) {
        File dir = usable(at) ? at : STORAGE;
        List<File> folders = children(dir);
        List<String> labels = new ArrayList<>();
        List<File> targets = new ArrayList<>();
        labels.add("Use this folder");
        targets.add(dir);
        File up = parent(dir);
        if (up != null) {
            labels.add(".. up to " + up.getName());
            targets.add(up);
        }
        for (File child : folders) {
            labels.add(child.getName() + (looksLikeGame(child) ? "  (game here)" : "") + "/");
            targets.add(child);
        }
        new AlertDialog.Builder(activity)
            .setTitle(dir.getAbsolutePath())
            .setItems(labels.toArray(new String[0]), (dialog, which) -> {
                File target = targets.get(which);
                if (which == 0) {
                    chosen.folder(target);
                } else {
                    show(activity, target, chosen);
                }
            })
            .setNegativeButton("Cancel", null)
            .show();
    }

    /** The folder to open on: the one already chosen, its nearest existing parent, or storage. */
    static File startAt(String chosen) {
        if (chosen == null || chosen.isEmpty()) {
            return STORAGE;
        }
        File at = new File(chosen);
        // data-path.txt names the Data folder; its parent is the install, which is the more
        // useful place to land.
        if ("Data".equalsIgnoreCase(at.getName()) && at.getParentFile() != null) {
            at = at.getParentFile();
        }
        while (at != null && !at.isDirectory()) {
            at = at.getParentFile();
        }
        return at == null || !usable(at) ? STORAGE : at;
    }

    /** Whether a path is one this browser shows: inside {@code /storage}, and a directory. */
    static boolean usable(File dir) {
        if (dir == null || !dir.isDirectory()) {
            return false;
        }
        String path = dir.getAbsolutePath();
        return path.equals(STORAGE.getPath()) || path.startsWith(STORAGE.getPath() + "/");
    }

    /** The folder above this one, or null at {@code /storage}, which is as far up as it goes. */
    static File parent(File dir) {
        File up = dir.getParentFile();
        return usable(up) ? up : null;
    }

    /** The subfolders of {@code dir}, by name, hidden ones left out. Empty when unreadable. */
    static List<File> children(File dir) {
        File[] all = dir.listFiles();
        if (all == null) {
            return new ArrayList<>();
        }
        List<File> found = new ArrayList<>();
        for (File child : all) {
            if (child.isDirectory() && !child.getName().startsWith(".")) {
                found.add(child);
            }
        }
        found.sort(Comparator.comparing(f -> f.getName().toLowerCase(java.util.Locale.ROOT)));
        return found.size() > MAX_ENTRIES ? new ArrayList<>(found.subList(0, MAX_ENTRIES)) : found;
    }

    /**
     * Whether a folder looks like it holds the game, so the list can say so. A hint for the
     * player only: {@code crate::odin::choose_folder} is what decides.
     */
    static boolean looksLikeGame(File dir) {
        return Arrays.asList("FalloutNV.esm", "Data/FalloutNV.esm").stream()
            .anyMatch(tail -> new File(dir, tail).isFile());
    }
}
