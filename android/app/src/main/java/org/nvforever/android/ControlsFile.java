package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * {@code nvforever-controls.txt} in the shared NVForever folder: one {@code name value} a
 * line, {@code #} starts a comment. The engine reads it at start
 * ({@code nvf-android/src/controller.rs}), so every switch here says "applied when the game
 * starts". The key names and the format are unchanged from the first Android build.
 *
 * <p>Plain Java on purpose, so it is tested with javac and java alone.
 */
public final class ControlsFile {
    /** Physical controller input. */
    public static final String CONTROLLER = "controller";
    /** On-screen touch controls. */
    public static final String TOUCH = "touch";
    /** Logs every native input event. */
    public static final String DIAGNOSTICS = "diagnostics";

    /** The crash tests, in the order the Debug screen shows them. */
    public static final String SKIP_PIPBOY_WARMUP = "skip-pipboy-warmup";
    public static final String SKIP_VATS_OUTLINE = "skip-vats-outline";
    public static final String VULKAN_VALIDATION = "vulkan-validation";
    public static final String VULKAN_VALIDATION_SYNC = "vulkan-validation-sync";
    public static final String VULKAN_VALIDATION_GPUAV = "vulkan-validation-gpuav";
    public static final String GPU_BREADCRUMBS = "gpu-breadcrumbs";
    public static final String GPU_BREADCRUMB_CHUNKS = "gpu-breadcrumb-chunks";
    public static final String NO_SOFT_PARTICLES = "no-soft-particles";
    public static final String NO_PARTICLES = "no-particles";
    public static final String NO_DECALS = "no-decals";
    public static final String NO_HEAT_HAZE = "no-heat-haze";

    /**
     * Every switch that makes the game slower or stops it loading, so the launcher can count
     * the ones that are on and warn. {@link #DIAGNOSTICS} is one of them: it writes big logs.
     */
    public static final String[] CRASH_TESTS = {
        SKIP_PIPBOY_WARMUP, SKIP_VATS_OUTLINE, VULKAN_VALIDATION, VULKAN_VALIDATION_SYNC,
        VULKAN_VALIDATION_GPUAV, GPU_BREADCRUMBS, GPU_BREADCRUMB_CHUNKS, NO_SOFT_PARTICLES,
        NO_PARTICLES, NO_DECALS, NO_HEAT_HAZE, DIAGNOSTICS,
    };

    private ControlsFile() {}

    /** The defaults the engine itself uses when a key is absent. */
    public static Map<String, Boolean> defaults() {
        Map<String, Boolean> values = new LinkedHashMap<>();
        values.put(CONTROLLER, false);
        values.put(TOUCH, true);
        for (String key : CRASH_TESTS) {
            values.put(key, false);
        }
        return values;
    }

    /** The file's values over {@link #defaults()}. An unreadable file leaves the defaults. */
    public static Map<String, Boolean> read(File file) {
        Map<String, Boolean> values = defaults();
        List<String> lines;
        try {
            lines = Files.readAllLines(file.toPath(), StandardCharsets.UTF_8);
        } catch (IOException | RuntimeException e) {
            return values;
        }
        for (String line : lines) {
            String text = line.trim();
            if (text.isEmpty() || text.startsWith("#")) {
                continue;
            }
            String[] pair = text.split("\\s+");
            if (pair.length < 2) {
                continue;
            }
            Boolean on = parse(pair[1]);
            if (on != null && values.containsKey(pair[0])) {
                values.put(pair[0], on);
            }
        }
        return values;
    }

    /** {@code true}, {@code false}, {@code 1} and {@code 0}; anything else is not a state. */
    private static Boolean parse(String word) {
        if (word.equals("true") || word.equals("1")) {
            return Boolean.TRUE;
        }
        if (word.equals("false") || word.equals("0")) {
            return Boolean.FALSE;
        }
        return null;
    }

    /** The whole file's text for these values. */
    public static String format(Map<String, Boolean> values) {
        StringBuilder sb = new StringBuilder("# Applied when starting the game\n");
        for (Map.Entry<String, Boolean> e : values.entrySet()) {
            sb.append(e.getKey()).append(' ').append(e.getValue()).append('\n');
        }
        return sb.toString();
    }

    /** Writes it whole, through a temporary file beside it. */
    public static void write(File file, Map<String, Boolean> values) throws IOException {
        Profiles.writeWhole(file, format(values));
    }

    /** The crash tests that are on, in the order of {@link #CRASH_TESTS}. */
    public static List<String> testsOn(Map<String, Boolean> values) {
        List<String> on = new ArrayList<>();
        for (String key : CRASH_TESTS) {
            if (Boolean.TRUE.equals(values.get(key))) {
                on.add(key);
            }
        }
        return on;
    }

    /** Every crash test off; the controller and touch choices are left alone. */
    public static void clearTests(Map<String, Boolean> values) {
        for (String key : CRASH_TESTS) {
            values.put(key, false);
        }
    }
}
