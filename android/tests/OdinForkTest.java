package org.nvforever.android;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;

/** Run directly with javac/java; these checks need neither Android nor game data. */
public final class OdinForkTest {
    public static void main(String[] args) throws Exception {
        Path temp = Files.createTempDirectory("nvf-odin-fork-test-");
        try {
            File files = temp.toFile();
            // The home is the folder every install of the fork already has, never upstream's.
            require(OdinFork.HOME.equals("NVForever"), "the fork's home folder name");
            require(OdinFork.home(files).getName().equals("NVForever"), "the home under the files directory");
            require(!OdinFork.home(files).getName().equals("home"), "upstream's name would hide the saves");

            // No data-path.txt: upstream's importer is what runs, and nothing is read in place.
            require(OdinFork.dataPath(files) == null, "no data path without the file");
            require(!OdinFork.readsInPlace(files), "an install with no data path imports a copy");

            File marker = OdinFork.dataPathFile(files);
            require(marker.getName().equals("data-path.txt"), "the data path file's name");

            write(marker, "/storage/emulated/0/NVForever/Data\n");
            require("/storage/emulated/0/NVForever/Data".equals(OdinFork.dataPath(files)), "the chosen folder");
            require(OdinFork.readsInPlace(files), "a data path means reading in place");

            // Surrounding space, a comment line and CRLF are all ordinary in a file the
            // player may have edited by hand.
            write(marker, "# chosen in the launcher\r\n   /storage/1234-5678/NVForever/Data   \r\n/ignored\r\n");
            require("/storage/1234-5678/NVForever/Data".equals(OdinFork.dataPath(files)), "a comment and trailing space");

            // Only an absolute path: a relative one would be a different folder in every process.
            for (String bad : new String[]{"", "   \n", "Data\n", "../Data\n", "/a\0b\n"}) {
                write(marker, bad);
                require(OdinFork.dataPath(files) == null, "refuses " + bad.trim());
            }

            // A file too large to be one path is refused rather than read into memory.
            StringBuilder huge = new StringBuilder("/storage/emulated/0/NVForever/Data\n");
            while (huge.length() <= 4096) huge.append("# padding padding padding padding padding\n");
            write(marker, huge.toString());
            require(OdinFork.dataPath(files) == null, "refuses an oversized data path file");

            // A directory in its place is not a path either.
            require(marker.delete() && marker.mkdir(), "replaces the file with a directory");
            require(OdinFork.dataPath(files) == null, "refuses a directory");
        } finally {
            try (var paths = Files.walk(temp)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
        System.out.println("OdinFork checks passed");
    }

    private static void write(File file, String text) throws Exception {
        Files.write(file.toPath(), text.getBytes(StandardCharsets.UTF_8));
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
