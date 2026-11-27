package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * The fork's crash-test switches as the launcher sees them: one table, one reader, no Android
 * types, so the launcher logic tests compile and run this (docs/fork.md, docs/odin-debug.md).
 *
 * <p>The owner's AYN Odin 3 loses its graphics device in play, and narrowing that down means
 * turning one drawn feature off at a time on the device itself. {@link OdinAdvanced} is the
 * page that shows these rows; this class is what a row means and what the file holds.
 *
 * <p>The file is {@code <home>/odin-advanced.txt}, one {@code key value} line per switch that
 * is on. The engine reads the same file and the same keys before it starts any threads
 * ({@code engine/nvforever/src/odin/crash_tests.rs}); the two tables are kept the same by
 * {@code tools/android/tests/test_fork.py}, because they cannot be shared at build time.
 *
 * <p>Every switch is off by default and gates only its own feature. A game with none of them
 * on draws exactly what it drew before.
 */
final class OdinCrashTests {
    /** The file in the engine home the page writes and the engine reads. */
    static final String FILE = "odin-advanced.txt";

    /** One row of the page: its file key, what it is called, what it does, what it needs. */
    static final class Switch {
        final String key;
        final String label;
        final String note;
        /** The {@link #key} of the switch this row needs, or null. Greyed until that is on. */
        final String needs;

        Switch(String key, String label, String note, String needs) {
            this.key = key;
            this.label = label;
            this.note = note;
            this.needs = needs;
        }
    }

    /**
     * Every switch, in the order the page lists them: the suspects first, then the
     * diagnostics that narrow a loss down, then the checks too slow to leave on.
     */
    static final Switch[] SWITCHES = {
        new Switch("no-vats-outline", "No V.A.T.S. target outline",
            "The selected body part is not lit on the target, so the outline's extra camera never draws. Targeting and the V.A.T.S. menu are unchanged.", null),
        new Switch("no-soft-particles", "No soft particles",
            "A particle has a hard edge where it passes into a wall, and its shader reads the depth prepass nowhere.", null),
        new Switch("no-particles", "No particles",
            "No particle system and no effect particle is drawn. They are still simulated, so nothing else in the game changes.", null),
        new Switch("no-decals", "No decals",
            "No decal is placed (blood, bullet holes). The shots and their impacts are unchanged.", null),
        new Switch("no-heat-haze", "No heat haze",
            "The heat-haze pass records nothing.", null),
        new Switch("no-pipboy-warm", "Skip the Pip-Boy warm-up during loads",
            "The hidden raise that warms the Pip-Boy's materials behind a loading screen is left out. It is the one thing drawing while a load streams.", null),
        new Switch("gpu-breadcrumbs", "GPU breadcrumbs",
            "Every render-graph node writes a marker before and after its work, so a lost device names the node the GPU hung in. A little slower.", null),
        new Switch("gpu-breadcrumb-chunks", "GPU breadcrumbs inside the transparent pass",
            "Cuts the transparent pass into up to 32 render passes with a marker between them, so a loss names the draws rather than the pass. Much slower.", "gpu-breadcrumbs"),
        new Switch("vulkan-validation", "Vulkan validation (slow)",
            "The Khronos validation layer checks every call and writes what it finds to the run log. Expect a much slower game.", null),
        new Switch("vulkan-validation-sync", "Vulkan sync validation",
            "Asks the layer for synchronization validation as well: a missing or wrong barrier between passes.", "vulkan-validation"),
        new Switch("vulkan-validation-gpuav", "Vulkan GPU-assisted validation (very slow)",
            "Asks the layer to instrument the shaders, so an out-of-bounds descriptor or buffer read is caught on the GPU. A save load may not finish.", "vulkan-validation"),
    };

    private OdinCrashTests() {
    }

    /** Where the switches are kept: inside the engine home, beside the saves and the logs. */
    static File file(File home) {
        return new File(home, FILE);
    }

    /**
     * The keys {@code text} turns on, in {@link #SWITCHES} order. A value of {@code true} or
     * {@code 1} is on; a blank line, a {@code #} comment, a line without a value and a key no
     * build of the fork has are all ignored, so a file an older or newer launcher wrote is
     * read for what this build understands and no more.
     */
    static Set<String> parse(String text) {
        Set<String> named = new LinkedHashSet<>();
        if (text != null) {
            for (String line : text.split("\\R")) {
                String trimmed = line.trim();
                if (trimmed.isEmpty() || trimmed.startsWith("#")) continue;
                String[] parts = trimmed.split("\\s+");
                if (parts.length >= 2 && (parts[1].equals("true") || parts[1].equals("1"))) named.add(parts[0]);
            }
        }
        Set<String> on = new LinkedHashSet<>();
        for (Switch s : SWITCHES) {
            if (named.contains(s.key)) on.add(s.key);
        }
        return on;
    }

    /** {@link #parse} of the file in {@code home}; an absent or unreadable file is no switch. */
    static Set<String> read(File home) {
        File file = file(home);
        if (!file.isFile() || file.length() > 65536) return new LinkedHashSet<>();
        try {
            return parse(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
        } catch (IOException | RuntimeException e) {
            return new LinkedHashSet<>();
        }
    }

    /**
     * What a file holding exactly {@code on} looks like: a header saying what the file is,
     * then one {@code key value} line per switch in {@link #SWITCHES} order. Only the switches
     * that are on are written, so the engine's reader never has to tell an off switch from an
     * absent one, and a build that drops a switch simply stops writing it.
     */
    static String render(Set<String> on) {
        StringBuilder out = new StringBuilder();
        out.append("# NVForever Android: the crash-test switches the launcher's Advanced page set.\n");
        out.append("# Written by the launcher, read by the engine at start. Delete this file to clear them.\n");
        for (Switch s : SWITCHES) {
            if (on.contains(s.key)) out.append(s.key).append(" true\n");
        }
        return out.toString();
    }

    /**
     * Write {@code on} into the file in {@code home}, creating the home if it is not there
     * yet. Nothing else in the home is touched.
     */
    static void write(File home, Set<String> on) throws IOException {
        if (!home.isDirectory() && !home.mkdirs()) throw new IOException("cannot make " + home);
        Files.write(file(home).toPath(), render(on).getBytes(StandardCharsets.UTF_8));
    }

    /** The row {@code key} names, or null for a key this build has no row for. */
    static Switch find(String key) {
        for (Switch s : SWITCHES) {
            if (s.key.equals(key)) return s;
        }
        return null;
    }

    /**
     * Whether {@code row} can be used at all: a row that needs another switch is greyed until
     * that one is on, because on its own it does nothing.
     */
    static boolean usable(Switch row, Set<String> on) {
        return row.needs == null || on.contains(row.needs);
    }

    /**
     * {@code on} without the switches whose {@link Switch#needs} is no longer satisfied,
     * repeatedly, so turning a switch off also turns off what depended on it. Turning
     * validation off therefore cannot leave sync validation on behind it.
     */
    static Set<String> settled(Set<String> on) {
        Set<String> kept = new LinkedHashSet<>(on);
        boolean changed = true;
        while (changed) {
            changed = false;
            List<String> drop = new ArrayList<>();
            for (String key : kept) {
                Switch row = find(key);
                if (row == null || !usable(row, kept)) drop.add(key);
            }
            if (!drop.isEmpty()) {
                kept.removeAll(drop);
                changed = true;
            }
        }
        return kept;
    }

    /**
     * What the launcher says beside the build line while any switch is on, or an empty string
     * for an ordinary install: nobody should play for a week wondering why the game stutters.
     */
    static String summary(Set<String> on) {
        int count = settled(on).size();
        if (count == 0) return "";
        return count == 1 ? "1 crash test on" : count + " crash tests on";
    }
}
