package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Map;

/**
 * The launcher's own logic, with no Android around it: the data-profile store, the Data
 * folder check, the controls file and what the active profile's folders say.
 *
 * <p>Run the way CI does:
 * <pre>
 * javac -d out android/app/src/main/java/org/nvforever/android/{DataCheck,Profiles,ControlsFile,RunState}.java android/tests/LauncherTest.java
 * java -cp out org.nvforever.android.LauncherTest
 * </pre>
 */
public final class LauncherTest {
    public static void main(String[] args) throws Exception {
        Path dir = Files.createTempDirectory("nvf-launcher-test");
        try {
            profilesStartFromTheExistingInstall(dir);
            profilesKeepTheVanillaHomeAndAddTheirOwn(dir);
            profilesForgetRemovesNothingFromDisk(dir);
            savesAreCopiedNotMoved(dir);
            dataCheckUsesTheEnginesOwnLists(dir);
            controlsFileRoundTrips(dir);
            lastRunFindsACrashOfTheNewestRun(dir);
        } finally {
            Files.walk(dir).sorted(Comparator.reverseOrder()).map(Path::toFile).forEach(File::delete);
        }
        System.out.println("LauncherTest: every check passed");
    }

    /** An install made before profiles existed becomes exactly one profile, named Vanilla. */
    private static void profilesStartFromTheExistingInstall(Path root) throws Exception {
        File files = fresh(root, "existing");
        Files.write(new File(files, "data-path.txt").toPath(),
                "/storage/emulated/0/Games/FNV/Data\n".getBytes(StandardCharsets.UTF_8));
        Profiles p = Profiles.load(files, new File("/storage/emulated/0/NVForever/Data"));
        check(p.size() == 1, "one profile");
        check(p.active().id.equals(Profiles.DEFAULT_ID), "its id is vanilla");
        check(p.active().data.equals("/storage/emulated/0/Games/FNV/Data"), "it keeps the chosen folder");
        check(p.active().home.equals(Profiles.DEFAULT_HOME), "and the home the install already has");
        check(p.active().homeDir(files).equals(new File(files, "NVForever")), "which is files/NVForever");
        check(p.active().savesDir(files).equals(new File(files, "NVForever/saves")), "so the saves do not move");

        // With no data-path.txt either, the engine's own default folder is the profile's.
        File empty = fresh(root, "empty");
        Profiles fallback = Profiles.load(empty, new File("/storage/emulated/0/NVForever/Data"));
        check(fallback.active().data.equals(new File("/storage/emulated/0/NVForever/Data").getPath()),
                "the default Data folder");
    }

    /** A second profile gets a home of its own, and the first one's home is untouched. */
    private static void profilesKeepTheVanillaHomeAndAddTheirOwn(Path root) throws Exception {
        File files = fresh(root, "two");
        // What an existing install has on disk, which must survive everything below.
        File vanillaSaves = new File(files, "NVForever/saves");
        vanillaSaves.mkdirs();
        Files.write(new File(vanillaSaves, "Autosave.nvfsave").toPath(), "save".getBytes(StandardCharsets.UTF_8));
        Files.write(new File(files, "NVForever/NVForever.ini").toPath(), "[Game]\n".getBytes(StandardCharsets.UTF_8));

        Profiles p = Profiles.load(files, new File("/sdcard/NVForever/Data"));
        Profiles.Profile modded = p.add("My Mods!", new File("/sdcard/NVForever/Data-modded"));
        check(modded.id.equals("my-mods"), "the id is a slug of the name, got " + modded.id);
        check(modded.home.equals("profiles/my-mods"), "its home is its own, got " + modded.home);
        p.setActive(modded.id);
        p.save(files);

        // The two pointers the engine reads, and nothing else.
        check(line(files, "data-path.txt").equals(new File("/sdcard/NVForever/Data-modded").getPath()),
                "data-path.txt is the active folder");
        check(line(files, "profile.txt").equals("profiles/my-mods"), "profile.txt is the active home");
        check(new File(files, "profiles.txt").isFile(), "the list is stored");

        // Read it back: the stored list is the same list.
        Profiles again = Profiles.load(files, new File("/sdcard/NVForever/Data"));
        check(again.size() == 2 && again.active().id.equals("my-mods"), "the list and the active one survive a reload");
        check(again.find(Profiles.DEFAULT_ID).home.equals(Profiles.DEFAULT_HOME), "Vanilla still points at files/NVForever");

        // The existing install's save and settings are exactly where they were.
        check(new File(vanillaSaves, "Autosave.nvfsave").isFile(), "the existing save was not moved");
        check(new File(files, "NVForever/NVForever.ini").isFile(), "the existing settings were not moved");
        check(!new File(files, "profiles/vanilla").exists(), "no home was created for Vanilla");

        // A name that slugs to the reserved id does not steal the Vanilla home.
        Profiles.Profile clash = again.add("Vanilla", new File("/sdcard/other"));
        check(!clash.home.equals(Profiles.DEFAULT_HOME), "a second Vanilla gets its own home, got " + clash.home);
    }

    /** Forget removes a row from the list and touches nothing on disk. */
    private static void profilesForgetRemovesNothingFromDisk(Path root) throws Exception {
        File files = fresh(root, "forget");
        Profiles p = Profiles.load(files, new File("/sdcard/NVForever/Data"));
        Profiles.Profile modded = p.add("Modded", new File("/sdcard/Data-modded"));
        p.setActive(modded.id);
        p.save(files);
        File home = modded.homeDir(files);
        new File(home, "saves").mkdirs();
        Files.write(new File(home, "saves/Quicksave.nvfsave").toPath(), "save".getBytes(StandardCharsets.UTF_8));

        check(!p.forget(Profiles.DEFAULT_ID), "the first profile cannot be forgotten");
        check(p.forget(modded.id), "another one can");
        p.save(files);
        check(p.size() == 1 && p.active().id.equals(Profiles.DEFAULT_ID), "the active one falls back");
        check(new File(home, "saves/Quicksave.nvfsave").isFile(), "its saves are still on disk");
        check(line(files, "profile.txt").equals(Profiles.DEFAULT_HOME), "and the engine is pointed back at Vanilla");
    }

    private static void savesAreCopiedNotMoved(Path root) throws Exception {
        File files = fresh(root, "copy");
        File from = new File(files, "NVForever/saves");
        from.mkdirs();
        Files.write(new File(from, "Goodsprings.nvfsave").toPath(), "a".getBytes(StandardCharsets.UTF_8));
        Files.write(new File(from, "notes.txt").toPath(), "b".getBytes(StandardCharsets.UTF_8));
        File to = new File(files, "profiles/modded/saves");
        check(Profiles.copySaves(from, to) == 1, "only the saves are copied");
        check(new File(from, "Goodsprings.nvfsave").isFile(), "the original is left where it was");
        check(new File(to, "Goodsprings.nvfsave").isFile(), "and a copy is in the new profile");
        check(!new File(to, "notes.txt").exists(), "nothing else is copied");
        check(Profiles.copySaves(from, to) == 0, "an existing name is never overwritten");
    }

    /** The check is the engine's own lists, and a file of zero length counts as missing. */
    private static void dataCheckUsesTheEnginesOwnLists(Path root) throws Exception {
        check(DataCheck.REQUIRED_DATA_FILES.length == 8, "8 required files");
        check(DataCheck.REQUIRED_DATA_FILES[7].equals("Update.bsa"), "the seventh archive is Update.bsa");
        check(DataCheck.ADDON_MASTERS.length == 9, "9 add-on masters");

        File data = fresh(root, "data");
        DataCheck.Result empty = DataCheck.scan(data);
        check(!empty.ready() && empty.missing().size() == 8, "an empty folder misses everything");
        for (String name : DataCheck.REQUIRED_DATA_FILES) {
            Files.write(new File(data, name).toPath(), "x".getBytes(StandardCharsets.UTF_8));
        }
        DataCheck.Result full = DataCheck.scan(data);
        check(full.ready(), "every required file present");
        check(full.archivesFound() == 7 && full.archivesTotal() == 7, "7 of 7 archives");
        check(full.addonsFound() == 0, "no add-on is required");

        // An interrupted copy leaves a file of zero length, which the engine refuses.
        Files.write(new File(data, "Update.bsa").toPath(), new byte[0]);
        check(!DataCheck.scan(data).ready(), "a zero-length file counts as missing");
        Files.write(new File(data, "Update.bsa").toPath(), "x".getBytes(StandardCharsets.UTF_8));

        // The files came from Windows, where case never mattered.
        Files.delete(new File(data, "FalloutNV.esm").toPath());
        Files.write(new File(data, "falloutnv.esm").toPath(), "x".getBytes(StandardCharsets.UTF_8));
        check(DataCheck.scan(data).baseMasterFound(), "the base master is found whatever its case");

        check(DataCheck.isClosedFolder(new File("/sdcard/Android/data/com.other/files/Data")), "Android/data is refused");
        check(DataCheck.isClosedFolder(new File("/sdcard/Android/obb")), "Android/obb is refused");
        check(!DataCheck.isClosedFolder(new File("/sdcard/NVForever/Data")), "an ordinary folder is not");

        // Picking the game's root folder means the Data inside it.
        File install = fresh(root, "install");
        File inner = new File(install, "Data");
        inner.mkdirs();
        Files.write(new File(inner, "FalloutNV.esm").toPath(), "x".getBytes(StandardCharsets.UTF_8));
        check(DataCheck.normalize(install).equals(inner), "the game root normalises to its Data folder");
        check(DataCheck.normalize(inner).equals(inner), "and a Data folder is left alone");
    }

    private static void controlsFileRoundTrips(Path root) throws Exception {
        File files = fresh(root, "controls");
        File file = new File(files, "nvforever-controls.txt");
        Map<String, Boolean> defaults = ControlsFile.read(file);
        check(Boolean.TRUE.equals(defaults.get(ControlsFile.TOUCH)), "touch controls are on by default");
        check(ControlsFile.testsOn(defaults).isEmpty(), "and no crash test is");

        Files.write(file.toPath(), ("# a comment\ncontroller true\ntouch 0\n"
                + "vulkan-validation 1\nnot-a-key true\ngpu-breadcrumbs maybe\n").getBytes(StandardCharsets.UTF_8));
        Map<String, Boolean> read = ControlsFile.read(file);
        check(Boolean.TRUE.equals(read.get(ControlsFile.CONTROLLER)), "true is on");
        check(Boolean.FALSE.equals(read.get(ControlsFile.TOUCH)), "0 is off");
        check(Boolean.TRUE.equals(read.get(ControlsFile.VULKAN_VALIDATION)), "1 is on");
        check(Boolean.FALSE.equals(read.get(ControlsFile.GPU_BREADCRUMBS)), "a word that is not a state is skipped");
        List<String> on = ControlsFile.testsOn(read);
        check(on.size() == 1 && on.get(0).equals(ControlsFile.VULKAN_VALIDATION), "one crash test is on");

        ControlsFile.write(file, read);
        check(ControlsFile.read(file).equals(read), "what is written is what is read back");
        check(!new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8).contains("not-a-key"),
                "an unknown key is never written back");
        ControlsFile.clearTests(read);
        check(ControlsFile.testsOn(read).isEmpty(), "every crash test off");
        check(Boolean.TRUE.equals(read.get(ControlsFile.CONTROLLER)), "and the controller choice is left alone");
    }

    private static void lastRunFindsACrashOfTheNewestRun(Path root) throws Exception {
        File logs = fresh(root, "logs");
        check(RunState.lastRun(logs) == RunState.LastRun.NONE, "no log yet");
        Files.write(new File(logs, "nvforever-20261119-120000-21.log").toPath(), "old".getBytes(StandardCharsets.UTF_8));
        Files.write(new File(logs, "crash-20261119-120001-21.txt").toPath(), "old".getBytes(StandardCharsets.UTF_8));
        check(RunState.lastRun(logs) == RunState.LastRun.CRASHED, "the newest run crashed");
        Files.write(new File(logs, "nvforever-20261120-120000-42.log").toPath(), "new".getBytes(StandardCharsets.UTF_8));
        check(RunState.lastRun(logs) == RunState.LastRun.OK, "a newer run with no crash of its own ended normally");
        Files.write(new File(logs, "crash-20261120-120002-42.txt").toPath(), "new".getBytes(StandardCharsets.UTF_8));
        check(RunState.lastRun(logs) == RunState.LastRun.CRASHED, "until its own crash report appears");

        File saves = fresh(root, "saves");
        check(RunState.newestSave(saves) == null && RunState.countSaves(saves) == 0, "no save yet");
        File older = new File(saves, "Quicksave.nvfsave");
        Files.write(older.toPath(), "a".getBytes(StandardCharsets.UTF_8));
        older.setLastModified(System.currentTimeMillis() - 86_400_000L);
        Files.write(new File(saves, "Goodsprings.nvfsave").toPath(), "b".getBytes(StandardCharsets.UTF_8));
        Files.write(new File(saves, "readme.txt").toPath(), "c".getBytes(StandardCharsets.UTF_8));
        check(RunState.countSaves(saves) == 2, "only saves are counted");
        check(RunState.newestSave(saves).name().equals("Goodsprings"), "the newest one, without its extension");
    }

    // ---- helpers ---------------------------------------------------------------------

    private static File fresh(Path root, String name) throws IOException {
        File dir = new File(root.toFile(), name);
        dir.mkdirs();
        return dir;
    }

    private static String line(File files, String name) throws IOException {
        return new String(Files.readAllBytes(new File(files, name).toPath()), StandardCharsets.UTF_8).trim();
    }

    private static void check(boolean ok, String what) {
        if (!ok) {
            throw new AssertionError(what);
        }
    }
}
