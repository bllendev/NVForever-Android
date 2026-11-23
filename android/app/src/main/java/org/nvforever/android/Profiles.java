package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Data profiles: a name, a Data folder, and a home folder of its own.
 *
 * <p>Everything a profile keeps falls out of its home, because the engine derives the plugin
 * list, the saves, the logs and the mods folder from {@code NVFOREVER_HOME}
 * ({@code fnv_install::EngineHome}). The launcher therefore owns exactly three files in the
 * app's private folder:
 *
 * <pre>
 * files/profiles.txt    this list (launcher only; the engine never reads it)
 * files/data-path.txt   the active profile's Data folder   (the existing contract)
 * files/profile.txt     the active profile's home, relative to files/   (read by paths.rs)
 * </pre>
 *
 * <p>The first profile is always "Vanilla", whose home is {@code files/NVForever}: the folder
 * every install has had since before profiles existed. An existing install therefore keeps its
 * settings, saves and logs exactly where they are, and nothing is moved or renamed. Only a
 * second profile ever creates {@code files/profiles/}.
 *
 * <p>The game's own settings ({@code NVForever.ini}) are deliberately NOT per profile: the
 * engine shares one copy across the homes at start ({@code paths::share_settings}).
 *
 * <p>The file is a line format, not JSON, so this class stays plain Java and is tested with
 * javac and java alone (Android's {@code org.json} is a stub outside a device).
 *
 * <pre>
 * active vanilla
 * profile vanilla|Vanilla|/storage/emulated/0/NVForever/Data|NVForever
 * profile modded|Modded|/storage/emulated/0/NVForever/Data-modded|profiles/modded
 * </pre>
 */
public final class Profiles {
    /** The id of the first profile, the one an install made before profiles existed becomes. */
    public static final String DEFAULT_ID = "vanilla";
    /** The home folder of that profile, relative to the app's private files folder. */
    public static final String DEFAULT_HOME = "NVForever";
    /** The engine's own default when nothing was chosen (nvf-android paths::data_dir). */
    public static final String DEFAULT_DATA_TAIL = "NVForever/Data";

    private static final String LIST_FILE = "profiles.txt";
    private static final String DATA_POINTER = "data-path.txt";
    private static final String HOME_POINTER = "profile.txt";
    private static final String HEADER =
            "# NVForever launcher data profiles. Written by the launcher; the engine reads\n"
            + "# data-path.txt and profile.txt beside it, never this file.\n"
            + "# profile <id>|<name>|<Data folder>|<home folder, relative to this one>\n";

    /** One profile. Its id is frozen at creation, because it is also a folder name. */
    public static final class Profile {
        public final String id;
        public String name;
        public String data;
        public final String home;

        Profile(String id, String name, String data, String home) {
            this.id = id;
            this.name = name;
            this.data = data;
            this.home = home;
        }

        /** The profile's Data folder. */
        public File dataDir() {
            return new File(data);
        }

        /** The profile's home: settings, plugins.txt, saves, logs. */
        public File homeDir(File filesDir) {
            return new File(filesDir, home);
        }

        public File savesDir(File filesDir) {
            return new File(homeDir(filesDir), "saves");
        }

        public File logsDir(File filesDir) {
            return new File(homeDir(filesDir), "logs");
        }

        public File pluginsFile(File filesDir) {
            return new File(homeDir(filesDir), "plugins.txt");
        }
    }

    private final List<Profile> list = new ArrayList<>();
    private String activeId = DEFAULT_ID;

    private Profiles() {}

    /**
     * The stored list, or a one-profile list built from what the install already has: the
     * folder in {@code data-path.txt} if there is one, else {@code defaultData}.
     */
    public static Profiles load(File filesDir, File defaultData) {
        Profiles p = new Profiles();
        for (String line : lines(new File(filesDir, LIST_FILE))) {
            String text = line.trim();
            if (text.isEmpty() || text.startsWith("#")) {
                continue;
            }
            if (text.startsWith("active ")) {
                p.activeId = text.substring(7).trim();
            } else if (text.startsWith("profile ")) {
                String[] parts = text.substring(8).split("\\|", -1);
                if (parts.length == 4 && !parts[0].trim().isEmpty() && !parts[3].trim().isEmpty()) {
                    p.list.add(new Profile(parts[0].trim(), parts[1].trim(), parts[2].trim(), parts[3].trim()));
                }
            }
        }
        if (p.list.isEmpty()) {
            String data = firstLine(new File(filesDir, DATA_POINTER));
            p.list.add(new Profile(DEFAULT_ID, "Vanilla",
                    data.isEmpty() ? defaultData.getPath() : data, DEFAULT_HOME));
            p.activeId = DEFAULT_ID;
        }
        if (p.find(p.activeId) == null) {
            p.activeId = p.list.get(0).id;
        }
        return p;
    }

    public List<Profile> all() {
        return java.util.Collections.unmodifiableList(list);
    }

    public int size() {
        return list.size();
    }

    public Profile active() {
        Profile p = find(activeId);
        return p == null ? list.get(0) : p;
    }

    public Profile find(String id) {
        for (Profile p : list) {
            if (p.id.equals(id)) {
                return p;
            }
        }
        return null;
    }

    /** Makes a profile the active one. Unknown ids are ignored. */
    public void setActive(String id) {
        if (find(id) != null) {
            activeId = id;
        }
    }

    /**
     * Adds a profile with a fresh id derived from its name. Returns it; the name is kept as
     * typed, only the id is a slug. The first profile's home is never reused.
     */
    public Profile add(String name, File dataDir) {
        String id = freeId(slug(name));
        Profile p = new Profile(id, name.trim().isEmpty() ? id : name.trim(), dataDir.getPath(), "profiles/" + id);
        list.add(p);
        return p;
    }

    /** Removes a profile from this list and nothing else: no folder, save or log is touched. */
    public boolean forget(String id) {
        if (DEFAULT_ID.equals(id) || list.size() < 2) {
            return false;
        }
        for (int i = 0; i < list.size(); i++) {
            if (list.get(i).id.equals(id)) {
                list.remove(i);
                if (activeId.equals(id)) {
                    activeId = list.get(0).id;
                }
                return true;
            }
        }
        return false;
    }

    public boolean rename(String id, String name) {
        Profile p = find(id);
        if (p == null || name.trim().isEmpty()) {
            return false;
        }
        p.name = name.trim();
        return true;
    }

    /** The text of {@code profiles.txt}. */
    public String format() {
        StringBuilder sb = new StringBuilder(HEADER);
        sb.append("active ").append(activeId).append('\n');
        for (Profile p : list) {
            sb.append("profile ").append(p.id).append('|').append(clean(p.name)).append('|')
                    .append(clean(p.data)).append('|').append(p.home).append('\n');
        }
        return sb.toString();
    }

    /**
     * Writes the list and the two pointers the engine reads. Written whole through a
     * temporary file, so a crash mid-write leaves the old list rather than half of one.
     */
    public void save(File filesDir) throws IOException {
        writeWhole(new File(filesDir, LIST_FILE), format());
        Profile a = active();
        writeWhole(new File(filesDir, DATA_POINTER), a.data + "\n");
        writeWhole(new File(filesDir, HOME_POINTER), a.home + "\n");
    }

    // ---- helpers ----------------------------------------------------------------------

    /** A name turned into a folder-safe id: lower case, letters and digits, dashes between. */
    public static String slug(String name) {
        StringBuilder sb = new StringBuilder();
        boolean dash = false;
        for (char c : name.trim().toLowerCase(Locale.ROOT).toCharArray()) {
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

    private String freeId(String wanted) {
        if (find(wanted) == null && !DEFAULT_ID.equals(wanted)) {
            return wanted;
        }
        for (int n = 2; ; n++) {
            String id = wanted + "-" + n;
            if (find(id) == null) {
                return id;
            }
        }
    }

    /** The field separator and the line end can never appear in a stored value. */
    private static String clean(String value) {
        return value.replace('|', ' ').replace('\n', ' ').replace('\r', ' ').trim();
    }

    private static List<String> lines(File file) {
        try {
            return Files.readAllLines(file.toPath(), StandardCharsets.UTF_8);
        } catch (IOException | RuntimeException e) {
            return new ArrayList<>();
        }
    }

    private static String firstLine(File file) {
        for (String line : lines(file)) {
            String text = line.trim();
            if (!text.isEmpty()) {
                return text;
            }
        }
        return "";
    }

    static void writeWhole(File file, String text) throws IOException {
        File parent = file.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        File temp = new File(file.getPath() + ".tmp");
        Files.write(temp.toPath(), text.getBytes(StandardCharsets.UTF_8));
        Files.move(temp.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING);
    }

    /**
     * Copies the saves of one profile into another's folder, skipping names already there.
     * A plain file copy: the folder copied from is left exactly as it was. Returns the count.
     */
    public static int copySaves(File fromSaves, File toSaves) throws IOException {
        File[] files = fromSaves.listFiles();
        if (files == null) {
            return 0;
        }
        toSaves.mkdirs();
        int copied = 0;
        for (File file : files) {
            if (!file.isFile() || !file.getName().toLowerCase(Locale.ROOT).endsWith(".nvfsave")) {
                continue;
            }
            File target = new File(toSaves, file.getName());
            if (target.exists()) {
                continue;
            }
            Files.copy(file.toPath(), target.toPath());
            copied++;
        }
        return copied;
    }
}
