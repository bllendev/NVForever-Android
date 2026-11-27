package org.nvforever.android;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.Set;

/** Run directly with javac/java; these checks need neither Android nor game data. */
public final class OdinCrashTestsTest {
    public static void main(String[] args) throws Exception {
        Path temp = Files.createTempDirectory("nvf-odin-crash-tests-");
        try {
            // The table names each switch once, says what it is, and needs only switches
            // that exist.
            Set<String> seen = new LinkedHashSet<>();
            for (OdinCrashTests.Switch s : OdinCrashTests.SWITCHES) {
                require(seen.add(s.key), s.key + " is listed twice");
                require(!s.label.isEmpty() && !s.note.isEmpty(), s.key + " has no words for the page");
                require(s.note.length() <= 200, s.key + "'s note does not fit the page");
                require(s.needs == null || OdinCrashTests.find(s.needs) != null, s.key + " needs a switch that is no switch");
            }
            require(OdinCrashTests.SWITCHES[0].key.equals("no-vats-outline"), "the page lists the suspect of the day first");
            require(OdinCrashTests.find("no-such-switch") == null, "a key no build has is no row");

            // The file: only the switches that are on, in the table's order, whatever order
            // they were named in.
            Set<String> on = set("no-decals", "no-vats-outline");
            String text = OdinCrashTests.render(on);
            require(text.startsWith("#"), "the file says what it is");
            require(text.indexOf("no-vats-outline true") < text.indexOf("no-decals true"), "the table's order");
            require(!text.contains("no-particles"), "an off switch is not written");
            require(OdinCrashTests.parse(text).equals(set("no-vats-outline", "no-decals")), "what was written is what is read");

            // A file a player or an older build may have left: comments, blank lines, CRLF,
            // a false value, a key with no value, a key this build has no row for.
            String odd = "# a comment\r\n\r\nno-vats-outline true\r\nno-decals false\r\n gpu-breadcrumbs 1 \r\nno-particles\r\nno-such-switch true\r\n";
            require(OdinCrashTests.parse(odd).equals(set("no-vats-outline", "gpu-breadcrumbs")), "reads what it understands");
            require(OdinCrashTests.parse("").isEmpty(), "an empty file is an ordinary install");
            require(OdinCrashTests.parse(null).isEmpty(), "so is no file at all");

            // On disk, including a home that is not there yet.
            File home = new File(temp.toFile(), "NVForever");
            require(OdinCrashTests.read(home).isEmpty(), "no file is no switch");
            OdinCrashTests.write(home, set("gpu-breadcrumbs", "gpu-breadcrumb-chunks"));
            require(OdinCrashTests.file(home).getName().equals("odin-advanced.txt"), "the file the engine reads");
            require(OdinCrashTests.read(home).equals(set("gpu-breadcrumbs", "gpu-breadcrumb-chunks")), "read back from the home");
            OdinCrashTests.write(home, new LinkedHashSet<>());
            require(OdinCrashTests.read(home).isEmpty(), "turning every switch off writes an empty list");

            // A row that needs another switch is unusable until that one is on, and turning
            // the one it needs off takes it with it rather than leaving it on behind it.
            OdinCrashTests.Switch chunks = OdinCrashTests.find("gpu-breadcrumb-chunks");
            require(!OdinCrashTests.usable(chunks, set()), "chunks need the breadcrumbs");
            require(OdinCrashTests.usable(chunks, set("gpu-breadcrumbs")), "with the breadcrumbs on");
            require(OdinCrashTests.settled(set("gpu-breadcrumb-chunks")).isEmpty(), "chunks alone do nothing");
            require(OdinCrashTests.settled(set("vulkan-validation-sync", "vulkan-validation-gpuav")).isEmpty(), "the deeper checks alone do nothing");
            require(OdinCrashTests.settled(set("vulkan-validation", "vulkan-validation-sync")).size() == 2, "both with the layer on");
            require(OdinCrashTests.settled(set("no-vats-outline")).equals(set("no-vats-outline")), "a switch that needs nothing stands alone");

            // What the launcher says beside the build line.
            require(OdinCrashTests.summary(set()).isEmpty(), "an ordinary install says nothing");
            require(OdinCrashTests.summary(set("no-vats-outline")).equals("1 crash test on"), "one");
            require(OdinCrashTests.summary(set("no-vats-outline", "no-decals")).equals("2 crash tests on"), "two");
            require(OdinCrashTests.summary(set("gpu-breadcrumb-chunks")).isEmpty(), "a switch that cannot apply is not counted");

            // That this table and the engine's are the same table is checked where the rest
            // of the fork's two-language values are: tools/android/tests/test_fork.py.
        } finally {
            try (var paths = Files.walk(temp)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
        System.out.println("OdinCrashTests checks passed");
    }

    private static Set<String> set(String... keys) {
        Set<String> out = new LinkedHashSet<>();
        for (String key : keys) out.add(key);
        return out;
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
