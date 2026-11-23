package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/**
 * Backing saves up to Download and reading them back, with no Android around it.
 *
 * <p>The two rules the class exists for are what this checks: a backup copies and never
 * moves or deletes, and a restore keeps a {@code .bak} of anything it writes over.
 *
 * <p>Run the way CI does:
 * <pre>
 * javac -d out android/app/src/main/java/org/nvforever/android/SaveBackup.java android/tests/SaveBackupTest.java
 * java -cp out org.nvforever.android.SaveBackupTest
 * </pre>
 */
public final class SaveBackupTest {
    public static void main(String[] args) throws Exception {
        Path dir = Files.createTempDirectory("nvf-save-backup-test");
        try {
            aBackupCopiesAndLeavesTheProfileAlone(dir);
            aSecondBackupIntoTheSameFolderReplacesNothing(dir);
            aRestoreKeepsABakOfEverySaveItWritesOver(dir);
            aSecondRestoreNeverOverwritesAnEarlierBak(dir);
            aRestoreIntoAnEmptyProfileClashesWithNothing(dir);
            backupsAreListedNewestFirst(dir);
            theFolderNameIsTheProfileAndTheDate(dir);
        } finally {
            Files.walk(dir).sorted(Comparator.reverseOrder()).map(Path::toFile).forEach(File::delete);
        }
        System.out.println("SaveBackupTest: every check passed");
    }

    /** A backup is a copy: every save is still in the profile, byte for byte, afterwards. */
    private static void aBackupCopiesAndLeavesTheProfileAlone(Path root) throws Exception {
        File saves = fresh(root, "copies/saves");
        write(saves, "Goodsprings.nvfsave", "one");
        write(saves, "Quicksave.nvfsave", "two");
        write(saves, "notes.txt", "not a save");
        File into = new File(root.toFile(), "copies/backup");

        check(SaveBackup.backUp(saves, into) == 2, "both saves were copied");
        check(SaveBackup.countSaves(into) == 2, "and only the saves");
        check(!new File(into, "notes.txt").exists(), "a file that is not a save is left behind");
        check(SaveBackup.countSaves(saves) == 2, "the profile still has both");
        check(read(saves, "Goodsprings.nvfsave").equals("one"), "unchanged");
        check(read(into, "Goodsprings.nvfsave").equals("one"), "and the copy is the same bytes");
    }

    /** Running a backup into a folder that already has one adds what is new and nothing else. */
    private static void aSecondBackupIntoTheSameFolderReplacesNothing(Path root) throws Exception {
        File saves = fresh(root, "again/saves");
        write(saves, "Goodsprings.nvfsave", "first");
        File into = new File(root.toFile(), "again/backup");
        check(SaveBackup.backUp(saves, into) == 1, "one save the first time");

        write(saves, "Goodsprings.nvfsave", "edited since");
        write(saves, "Novac.nvfsave", "new one");
        check(SaveBackup.backUp(saves, into) == 1, "only the new name is copied the second time");
        check(read(into, "Goodsprings.nvfsave").equals("first"), "the backup's own copy is not replaced");
        check(SaveBackup.countSaves(into) == 2, "and the backup now holds both");
    }

    /** A restore never loses a save: what it writes over is copied to .bak first. */
    private static void aRestoreKeepsABakOfEverySaveItWritesOver(Path root) throws Exception {
        File saves = fresh(root, "restore/saves");
        File backup = fresh(root, "restore/backup");
        write(saves, "Goodsprings.nvfsave", "the one on the phone");
        write(backup, "Goodsprings.nvfsave", "the one in the backup");
        write(backup, "Novac.nvfsave", "only in the backup");

        check(SaveBackup.clashes(backup, saves) == 1, "one name is in both");
        check(SaveBackup.restore(backup, saves) == 2, "both are restored");
        check(read(saves, "Goodsprings.nvfsave").equals("the one in the backup"), "the backup's copy wins");
        check(read(saves, "Goodsprings.nvfsave.bak").equals("the one on the phone"),
                "and the save it replaced is kept beside it");
        check(read(saves, "Novac.nvfsave").equals("only in the backup"), "the new one arrives");
        check(SaveBackup.countSaves(backup) == 2, "and the backup is untouched");
    }

    /** A second restore makes a .bak-2 rather than writing over the first .bak. */
    private static void aSecondRestoreNeverOverwritesAnEarlierBak(Path root) throws Exception {
        File saves = fresh(root, "twice/saves");
        File backup = fresh(root, "twice/backup");
        write(saves, "Save.nvfsave", "original");
        write(backup, "Save.nvfsave", "from the backup");

        SaveBackup.restore(backup, saves);
        SaveBackup.restore(backup, saves);
        check(read(saves, "Save.nvfsave.bak").equals("original"), "the first .bak is the original save");
        check(read(saves, "Save.nvfsave.bak-2").equals("from the backup"),
                "the second restore keeps its own copy under a free name");
    }

    /** Restoring into a profile with no save of that name writes no .bak at all. */
    private static void aRestoreIntoAnEmptyProfileClashesWithNothing(Path root) throws Exception {
        File saves = new File(root.toFile(), "empty/saves");
        File backup = fresh(root, "empty/backup");
        write(backup, "Save.nvfsave", "only copy");

        check(SaveBackup.clashes(backup, saves) == 0, "nothing clashes with an empty profile");
        check(SaveBackup.restore(backup, saves) == 1, "the save is written");
        check(!new File(saves, "Save.nvfsave.bak").exists(), "and no .bak is made for nothing");
    }

    /** The restore list is newest first, and a folder with no save in it is not a backup. */
    private static void backupsAreListedNewestFirst(Path root) throws Exception {
        File downloads = fresh(root, "download");
        File older = new File(SaveBackup.backupsRoot(downloads), "vanilla-20261101-1200");
        File newer = new File(SaveBackup.backupsRoot(downloads), "vanilla-20261123-1200");
        File empty = new File(SaveBackup.backupsRoot(downloads), "vanilla-empty");
        older.mkdirs();
        newer.mkdirs();
        empty.mkdirs();
        write(older, "A.nvfsave", "a");
        write(newer, "B.nvfsave", "b");
        older.setLastModified(System.currentTimeMillis() - 86_400_000L);

        List<File> backups = SaveBackup.backups(downloads);
        check(backups.size() == 2, "a folder with no save in it is not offered");
        check(backups.get(0).getName().equals(newer.getName()), "the newest one is first");
    }

    /** The folder a backup goes to names the profile and when it was taken. */
    private static void theFolderNameIsTheProfileAndTheDate(Path root) {
        File downloads = new File(root.toFile(), "download");
        File folder = SaveBackup.folderFor(downloads, "My Modded Run", 0L);
        check(folder.getParentFile().getName().equals(SaveBackup.BACKUPS_TAIL),
                "every backup is under the one folder");
        check(folder.getName().startsWith("my-modded-run-"), "named for the profile, folder-safe");
        check(folder.getName().length() > "my-modded-run-".length(), "and stamped with the date");
        check(SaveBackup.folderFor(downloads, "/../..", 0L).getName().startsWith("profile-"),
                "a name with nothing usable in it still cannot climb out of the folder");
    }

    // ---- helpers ---------------------------------------------------------------------

    private static File fresh(Path root, String name) {
        File dir = new File(root.toFile(), name);
        dir.mkdirs();
        return dir;
    }

    private static void write(File dir, String name, String text) throws IOException {
        Files.write(new File(dir, name).toPath(), text.getBytes(StandardCharsets.UTF_8));
    }

    private static String read(File dir, String name) throws IOException {
        return new String(Files.readAllBytes(new File(dir, name).toPath()), StandardCharsets.UTF_8);
    }

    private static void check(boolean ok, String what) {
        if (!ok) {
            throw new AssertionError(what);
        }
    }
}
