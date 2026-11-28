package org.nvforever.android;

import android.app.AlertDialog;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.os.SystemClock;
import android.text.TextUtils;
import android.view.Gravity;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.StringWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.text.DateFormat;
import java.util.Date;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * The rows the Android fork adds to upstream's launcher, and the gamepad that works them
 * (docs/fork.md).
 *
 * <p>Upstream's {@link MobileLauncher} is kept as the launcher: it has the news feed, the
 * settings catalogue, the mods editor, the save preview and the in-app updater, and this fork
 * is not going to grow a second one of those. What it does not have is what a handheld needs,
 * so this adds it, in upstream's own style and from a file of its own:
 *
 * <ul>
 *   <li><b>Use a folder in place</b> on the Game folder page, beside their importer, with a
 *       folder browser that yields a path ({@link OdinFolderPicker}, {@code crate::odin}).
 *   <li><b>Back up saves</b> and <b>Restore saves</b> to Download ({@link SaveBackup}).
 *   <li><b>Export debug log</b>, <b>Copy debug summary</b> and the line that says when the
 *       last run ended unexpectedly ({@link DebugReport}, {@code crate::odin::last_run_note}).
 *   <li><b>Load save</b>, a page of the saves newest first, which starts the game on the one
 *       the player picks through upstream's own one-start request.
 *   <li><b>Graphics driver</b> on upstream's Settings page: Automatic, System, each driver the
 *       player installed, and a zip picker that installs another ({@link GpuDrivers},
 *       {@code nvforever::odin::gpu_driver}).
 *   <li><b>A gamepad that works the whole launcher.</b> Upstream's has no {@code KeyEvent},
 *       {@code onKeyDown} or focus handling at all: it is touch only, which on a device with
 *       buttons is a daily annoyance. Touch behaviour is unchanged; nothing here is focusable
 *       in touch mode, so a tap never leaves a focus ring behind.
 * </ul>
 *
 * <p>Every string is hardcoded English, which is upstream's pattern for this launcher.
 */
final class OdinLauncher {
    /** How long one push of the stick waits before it moves the focus again. */
    private static final long STICK_REPEAT_MS = 180;
    /** Past this much of the stick, a push counts; under it, it is let go. */
    private static final float STICK_EDGE = 0.5f;
    /** How much of a report goes on the clipboard, which is not a place for 200 KiB. */
    private static final int SUMMARY_CHARS = 8000;
    /** Where a report goes when the device has no document picker. */
    private static final String REPORTS_TAIL = "NVForever-reports";
    /** How long the page waits before asking again while a game folder is being looked for. */
    private static final long RECHECK_MS = 400;

    /** The up and down arrows a plugin row is reordered with. */
    private static final String UP = "▲";
    private static final String DOWN = "▼";

    private final LauncherActivity activity;
    private final MobileLauncher ui;
    private JSONArray saveList;
    private String savesError = "";
    private String focusOn;
    private int stickHeld;
    private long stickMoved;

    /**
     * The density this launcher is drawn at: compact on a handheld, upstream's otherwise
     * ({@link OdinCompact}). Measured again before every render, because a controller can be
     * plugged in and the device can be turned.
     */
    private OdinCompact compact = OdinCompact.roomy();

    /** The columns the cards of this page are dealt into, and the row holding them. */
    private LinearLayout[] columns;
    private int columnNext;
    private int columnCount = 1;

    /** This render's missing masters, read once per render (crate::odin::masters). */
    private JSONArray masterProblems;

    OdinLauncher(LauncherActivity activity, MobileLauncher ui) {
        this.activity = activity;
        this.ui = ui;
        this.compact = measure();
    }

    // ---- the compact density ---------------------------------------------------------

    /** This screen, right now: its shape, its size and whether anything is plugged into it. */
    private OdinCompact measure() {
        android.content.res.Configuration config = activity.getResources().getConfiguration();
        boolean landscape = config.orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE;
        return OdinCompact.of(landscape, config.smallestScreenWidthDp, controllerPresent());
    }

    /** How wide the launcher is, in dp, which is how many columns it holds. */
    private int widthDp() {
        return activity.getResources().getConfiguration().screenWidthDp;
    }

    /** A text size, a padding, a row height and a gap, at this density (upstream's helpers). */
    int text(int size) {
        return compact.text(size);
    }

    int pad(int dp) {
        return compact.pad(dp);
    }

    int scale(int dp) {
        return compact.scale(dp);
    }

    int rowHeight() {
        return compact.rowHeight();
    }

    int gap() {
        return compact.gap();
    }

    /**
     * Where upstream's next card goes: straight into the page, or into the next of the
     * columns a compact page is dealt into. The columns grow apart, so a short card next to a
     * tall one wastes nothing.
     */
    LinearLayout cardParent(LinearLayout content) {
        if (columnCount <= 1) {
            return content;
        }
        if (columns == null) {
            LinearLayout row = ui.row();
            columns = new LinearLayout[columnCount];
            for (int i = 0; i < columnCount; i++) {
                columns[i] = ui.column();
                LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, -2, 1);
                lp.leftMargin = i == 0 ? 0 : ui.dp(gap());
                row.addView(columns[i], lp);
            }
            content.addView(row, new LinearLayout.LayoutParams(-1, -2));
        }
        return columns[columnNext++ % columnCount];
    }

    /** A heading goes across the page, and the cards under it start a fresh set of columns. */
    void heading(LinearLayout content, View view) {
        columns = null;
        columnNext = 0;
        ui.add(content, view);
    }

    // ---- the engine home's folders ---------------------------------------------------

    private File home() {
        return OdinFork.home(activity.getFilesDir());
    }

    private File savesDir() {
        return new File(home(), "saves");
    }

    private File logsDir() {
        return new File(home(), "logs");
    }

    private static File downloads() {
        return Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS);
    }

    // ---- the Game folder page --------------------------------------------------------

    /**
     * What the page says while no complete game folder is known. The look for one runs on a
     * thread of its own, because a storage card that has gone to sleep can take seconds to
     * answer, so until it finishes the page says so rather than saying there is nothing.
     */
    String noFolderLine() {
        return ui.state.optBoolean("inPlaceChecking", false)
            ? "Checking game folder..."
            : "No complete game folder is ready.";
    }

    /** Whether the launcher should ask again shortly, because the look is still running. */
    boolean stillChecking() {
        return ui.state.optBoolean("inPlaceChecking", false);
    }

    /** The row that chooses a folder to read in place, beside upstream's importer. */
    void chooseFolderRow(LinearLayout box) {
        ui.add(box, ui.label("Or read your game where it is: nothing is copied, no app storage is used, and a mod or a replacement BSA you drop in later is seen at once. This needs Android's All files access.", 15, ui.weak));
        ui.add(box, ui.tap("Use a folder in place", true, ui.editable(), this::pickInPlace));
        if (!OdinStorage.granted(activity)) {
            ui.add(box, ui.tap("Allow access to your game folder", false, true, () -> {
                OdinStorage.promptIfNeeded(activity);
            }));
        }
        ui.add(box, ui.tap("Look for a game folder again", false, ui.editable(), () -> {
            try {
                ui.send("odin", "ask", ask("look_again"));
                ui.message("Looking for a game folder...");
                ui.refresh(false, false, null);
            } catch (IOException | JSONException e) {
                ui.message(said(e, "Could not look again."));
            }
        }));
    }

    /** The rows shown once the game is read in place: change the folder, or import instead. */
    void inPlaceRows(LinearLayout box) {
        ui.add(box, ui.tap("Use a different folder", false, ui.editable(), this::pickInPlace));
    }

    /**
     * Ask again shortly while the look for a game folder is still running on its own thread,
     * so the page stops saying "Checking game folder..." by itself.
     */
    void afterRefresh() {
        if (!stillChecking() || activity.isDestroyed() || activity.isFinishing()) {
            return;
        }
        ui.root.postDelayed(() -> {
            if (!activity.isDestroyed() && !activity.isFinishing()) {
                ui.refresh(false, activity.runningGame(), null);
            }
        }, RECHECK_MS);
    }

    private void pickInPlace() {
        if (!OdinStorage.granted(activity)) {
            OdinStorage.promptIfNeeded(activity);
            return;
        }
        OdinFolderPicker.show(activity, OdinFolderPicker.startAt(OdinFork.dataPath(activity.getFilesDir())), dir -> {
            try {
                Object taken = ui.send("odin", "ask", ask("choose_folder", "path", dir.getAbsolutePath()));
                ui.refresh(false, false, "Reading your game in place at " + taken + ".");
            } catch (IOException | JSONException e) {
                ui.message(said(e, "That folder could not be used."));
            }
        });
    }

    // ---- the Home page ---------------------------------------------------------------

    /**
     * The fork's rows on Home: the last run, the saves tools and the debug report.
     *
     * <p>Compact, each card's words are the short form and its two buttons sit side by side,
     * so Report a problem is on the screen rather than under it. That is what the owner's
     * crash of 2026-11-27 cost him: the card was there, four rows down.
     */
    void homeRows(LinearLayout about) {
        String ended = ui.state.isNull("lastRunNote") ? "" : ui.state.optString("lastRunNote", "");
        LinearLayout report = ui.card(ended.isEmpty() ? "Report a problem" : "The last run ended unexpectedly");
        if (!ended.isEmpty()) {
            ui.add(report, ui.label(ended, 15, ui.text));
        }
        ui.add(report, ui.label(compact.on
            ? "One text file: this build, this device and the newest run log. Sent nowhere by itself."
            : "Writes one text file: this build and device, then the newest run log, bounded. No saves, no settings and no device-wide logs, and it is sent nowhere by itself. Review it before sharing.",
            compact.on ? 14 : 15, ui.weak));
        pair(report,
            ui.tap("Export debug log", !ended.isEmpty(), ui.editable(), this::exportLog),
            ui.tap("Copy debug summary", false, ui.editable(), this::copySummary));
        String fellBack = GpuDrivers.lastStartFellBack(activity.getFilesDir());
        if (fellBack != null) {
            LinearLayout driver = ui.card("Graphics driver");
            ui.add(driver, ui.label("The last start used the system graphics driver: " + fellBack, 15, ui.text));
            ui.add(driver, ui.label("The choice is in Settings, under Graphics.", 14, ui.weak));
        }
        LinearLayout saves = ui.card("Saves");
        ui.add(saves, ui.label(compact.on
            ? "Saves are in this app's own storage, which uninstalling deletes. A backup is a plain folder in Download."
            : "Saves live in this app's own storage, which no file app can reach and which uninstalling deletes. A backup is a plain folder of save files in Download, so it can be copied off the device with anything.",
            compact.on ? 14 : 15, ui.weak));
        pair(saves,
            ui.tap("Back up saves", false, ui.editable(), this::backUpSaves),
            ui.tap("Restore saves", false, ui.editable(), this::restoreSaves));
        ui.add(saves, ui.tap("Load save", false, ui.editable(), () -> goTo(MobileLauncher.Page.SAVES)));
        if (controllerPresent()) {
            ui.add(about, ui.label("Controller: A select, B back, LB and RB change page, Start plays, D-pad or left stick moves.", 14, ui.weak));
        }
    }

    /** Two buttons: side by side where the density is compact, one under the other where it is not. */
    private void pair(LinearLayout box, Button first, Button second) {
        if (!compact.on) {
            ui.add(box, first);
            ui.add(box, second);
            return;
        }
        LinearLayout row = ui.row();
        for (Button button : new Button[] { first, second }) {
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, -2, 1);
            lp.rightMargin = button == first ? ui.dp(gap()) : 0;
            row.addView(button, lp);
        }
        ui.add(box, row);
    }

    /** Open one of upstream's pages, as its own nav bar does. */
    private void goTo(MobileLauncher.Page page) {
        if (ui.page != page) {
            ui.page = page;
            ui.scroll.scrollTo(0, 0);
            ui.render();
        }
    }

    // ---- the Graphics driver choice --------------------------------------------------

    /**
     * The fork's card on upstream's Settings page: which Vulkan driver the next start uses
     * ({@link GpuDrivers}, docs/odin-drivers.md). One row per choice rather than a dialog, so
     * every one of them is a focusable row a gamepad reaches like any other.
     */
    void settingsRows(String tab) {
        if (!"Graphics".equals(tab)) {
            return;
        }
        File files = activity.getFilesDir();
        LinearLayout box = ui.card("Graphics driver");
        ui.add(box, ui.label("The game can run on a Vulkan driver you install yourself: a Mesa Turnip build for Adreno, which may be newer than the one in this phone's system image. NVForever ships none, and a driver whose first seconds do not survive is dropped by itself at the next start.", 15, ui.weak));
        String why = GpuDrivers.lastStartFellBack(files);
        if (why != null) {
            ui.add(box, ui.label("The last start used the system driver: " + why, 15, ui.text));
        }
        if (OdinCrashTests.settled(OdinCrashTests.read(home())).contains("vulkan-validation")) {
            ui.add(box, ui.label("The Vulkan validation crash test is on, and it runs on the system driver, so the next start uses that whatever is chosen here.", 15, ui.text));
        }
        String mode = GpuDrivers.mode(files);
        List<String[]> drivers = GpuDrivers.installed(files);
        driverChoice(box, "Automatic", GpuDrivers.AUTOMATIC.equals(mode), GpuDrivers.AUTOMATIC, null);
        driverChoice(box, "System", mode == null, null, null);
        for (String[] driver : drivers) {
            driverChoice(box, driver[1], driver[0].equals(mode), driver[0], driver[0]);
        }
        ui.add(box, ui.label(drivers.isEmpty()
            ? "No driver of your own is installed, so Automatic is the system driver. A driver zip is one you supply: a single 64-bit ARM .so, with meta.json if it has one."
            : "Hold a driver to remove it.", 14, ui.weak));
        ui.add(box, ui.tap("Install driver zip...", false, ui.editable(), this::pickDriverZip));
    }

    /** One row of the choice: tapped it becomes the next start's driver, held it is removed. */
    private void driverChoice(LinearLayout box, String label, boolean chosen, String id, String removable) {
        Button row = ui.tap(label, chosen, ui.editable(), () -> {
            GpuDrivers.select(activity.getFilesDir(), id);
            ui.refresh(false, false, "The next start uses " + label + ".");
        });
        if (removable != null) {
            row.setLongClickable(true);
            row.setOnLongClickListener(v -> {
                confirmRemoveDriver(removable, label);
                return true;
            });
        }
        ui.add(box, row);
    }

    private void pickDriverZip() {
        Intent open = new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .putExtra(Intent.EXTRA_MIME_TYPES, new String[] { "application/zip", "application/octet-stream" });
        try {
            activity.startActivityForResult(open, LauncherActivity.ODIN_INSTALL_DRIVER);
        } catch (android.content.ActivityNotFoundException e) {
            ui.message("This device has no document picker, so a driver zip cannot be chosen here.");
        }
    }

    /** The document picker came back. A null URI means the player left it. */
    void installDriver(Uri uri) {
        if (uri == null) {
            return;
        }
        File files = activity.getFilesDir();
        String id;
        try (InputStream zip = activity.getContentResolver().openInputStream(uri)) {
            if (zip == null) {
                throw new IOException("that file could not be opened");
            }
            id = GpuDrivers.install(files, zip);
        } catch (IOException | RuntimeException e) {
            ui.message(said(e, "That driver could not be installed."));
            return;
        }
        String label = GpuDrivers.label(new File(GpuDrivers.root(files), id));
        ui.refresh(false, false, GpuDrivers.AUTOMATIC.equals(GpuDrivers.mode(files))
            ? label + " is installed, and Automatic now uses it."
            : label + " is installed. Choose it under Graphics driver to use it.");
    }

    private void confirmRemoveDriver(String id, String label) {
        new AlertDialog.Builder(activity)
            .setTitle("Remove " + label)
            .setMessage("Delete this driver from the app's own storage. Your saves and your game folder are untouched, and the system driver is always there.")
            .setPositiveButton("Remove", (dialog, which) -> {
                GpuDrivers.delete(activity.getFilesDir(), id);
                ui.refresh(false, false, label + " is removed.");
            })
            .setNegativeButton("Cancel", null)
            .show();
    }

    // ---- the Load save page ----------------------------------------------------------

    /** The saves, newest first; picking one starts the game on it. */
    void savesPage() {
        LinearLayout box = ui.card("Load save");
        if (ui.state.isNull("folder")) {
            ui.add(box, ui.label("Set up a game folder first: a save can only be loaded by the game it was made in.", 15, ui.weak));
            return;
        }
        if (saveList == null) {
            try {
                saveList = ((JSONObject) ui.send("odin", "ask", ask("saves"))).getJSONArray("saves");
                savesError = "";
            } catch (IOException | JSONException | ClassCastException e) {
                saveList = new JSONArray();
                savesError = said(e, "The save list could not be read.");
            }
        }
        if (!savesError.isEmpty()) {
            ui.add(box, ui.label(savesError, 15, ui.weak));
            return;
        }
        if (saveList.length() == 0) {
            ui.add(box, ui.label("No saves yet. Play, then save from the pause menu.", 15, ui.weak));
            return;
        }
        ui.add(box, ui.label("Newest first. Picking one starts the game on that save.", 15, ui.weak));
        for (int i = 0; i < saveList.length(); i++) {
            JSONObject save = saveList.optJSONObject(i);
            if (save == null) {
                continue;
            }
            String name = save.optString("name", "");
            ui.add(box, ui.tap(name, i == 0, ui.editable(), () -> activity.prepareAndStartSave(name)));
            ui.add(box, ui.label(saveDetail(save), 14, ui.weak));
        }
    }

    /** Forget the save list, so the page reads it again (a start, or a restore, changes it). */
    void savesChanged() {
        saveList = null;
        savesError = "";
    }

    /** One save's line: when it was saved, who by and how big the file is. */
    static String saveDetail(JSONObject save) {
        long when = save.optLong("savedUnix", 0L);
        String time = when > 0
            ? DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT, Locale.getDefault())
                .format(new Date(when * 1000L))
            : "time unknown";
        String who = save.optString("player", "");
        int level = save.optInt("level", 0);
        String person = who.isEmpty() ? "" : who + (level > 0 ? " level " + level : "");
        return time + (person.isEmpty() ? "" : "  ·  " + person) + "  ·  " + bytes(save.optLong("size", 0L));
    }

    /** A file's size, as a player reads one. */
    static String bytes(long size) {
        if (size >= 1024L * 1024L) {
            return String.format(Locale.ROOT, "%.1f MB", size / (1024.0 * 1024.0));
        }
        if (size >= 1024L) {
            return (size / 1024L) + " KB";
        }
        return size + " bytes";
    }

    // ---- the Mods page: the load order, and the masters it is missing ----------------

    /**
     * The load order, one slim row per plugin: the checkbox that turns it on, its name, and
     * two arrows that move it. Upstream drew a full-width checkbox and then a full-width Move
     * up and Move down under it, which is three rows of 48 dp per plugin and four plugins on a
     * handheld screen. These are one row of {@link OdinCompact#rowHeight()}, dealt into two
     * columns where the screen is wide enough, in load order down one column and then down the
     * next.
     *
     * <p>Above them, when any enabled plugin names a master that will not load, is what the
     * start would be refused for and the one tap that clears it (crate::odin::masters).
     */
    void pluginRows(LinearLayout box, JSONArray items, boolean haveGame) throws JSONException {
        JSONArray problems = missingMasters();
        if (problems.length() > 0) {
            LinearLayout warning = ui.row();
            warning.setGravity(Gravity.CENTER_VERTICAL);
            warning.addView(ui.label(missingSummary(problems), 15, ui.gold),
                new LinearLayout.LayoutParams(0, -2, 1));
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-2, -2);
            lp.leftMargin = ui.dp(gap());
            warning.addView(ui.tap("Turn off plugins with missing masters", true, ui.editable(), this::turnOffMissing), lp);
            ui.add(box, warning);
        }
        int count = items.length();
        int wanted = compact.pluginColumns(widthDp());
        int first = OdinCompact.firstColumn(count, wanted);
        LinearLayout left = box;
        LinearLayout right = box;
        if (wanted > 1 && count > 1) {
            LinearLayout row = ui.row();
            left = ui.column();
            right = ui.column();
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, -2, 1);
            lp.rightMargin = ui.dp(gap());
            row.addView(left, lp);
            row.addView(right, new LinearLayout.LayoutParams(0, -2, 1));
            box.addView(row, new LinearLayout.LayoutParams(-1, -2));
        }
        for (int i = 0; i < count; i++) {
            pluginRow(i < first ? left : right, items, i, haveGame, problems);
        }
    }

    private void pluginRow(LinearLayout into, JSONArray items, int i, boolean haveGame, JSONArray problems)
            throws JSONException {
        JSONObject item = items.getJSONObject(i);
        String name = item.getString("name");
        boolean base = "FalloutNV.esm".equalsIgnoreCase(name);
        LinearLayout row = ui.row();
        row.setGravity(Gravity.CENTER_VERTICAL);
        CheckBox check = new CheckBox(activity);
        check.setText(name);
        check.setTextSize(text(15));
        check.setSingleLine(true);
        check.setEllipsize(TextUtils.TruncateAt.MIDDLE);
        check.setMinimumHeight(ui.dp(rowHeight()));
        check.setChecked(base || item.getBoolean("active"));
        check.setEnabled(haveGame && !base);
        check.setOnClickListener(v -> ui.changePlugin(name, "toggle"));
        row.addView(check, new LinearLayout.LayoutParams(0, -2, 1));
        if (!base) {
            boolean up = i > 0 && !"FalloutNV.esm".equalsIgnoreCase(items.getJSONObject(i - 1).getString("name"));
            boolean down = i + 1 < items.length()
                && !"FalloutNV.esm".equalsIgnoreCase(items.getJSONObject(i + 1).getString("name"));
            arrow(row, UP, "Move " + name + " up", haveGame && up, () -> ui.changePlugin(name, "up"));
            arrow(row, DOWN, "Move " + name + " down", haveGame && down, () -> ui.changePlugin(name, "down"));
        }
        // No gap between one plugin and the next: the rows are a list, and 4 dp each over a
        // load order of fifty is a screenful.
        into.addView(row, new LinearLayout.LayoutParams(-1, -2));
        String needs = needs(problems, name);
        if (!needs.isEmpty()) {
            into.addView(ui.label(needs, 14, ui.gold), new LinearLayout.LayoutParams(-1, -2));
        }
    }

    /** One arrow of a plugin row: a square tap target with the row's own words behind it. */
    private void arrow(LinearLayout row, String glyph, String said, boolean enabled, Runnable click) {
        Button button = ui.tap(glyph, false, enabled, click);
        button.setContentDescription(said);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-2, -2);
        lp.leftMargin = ui.dp(gap());
        row.addView(button, lp);
    }

    /** Compact, the load order goes above the new game start, which is read far less often. */
    void afterMods(LinearLayout content) {
        if (!compact.on || content.getChildCount() < 2) {
            return;
        }
        View plugins = content.getChildAt(content.getChildCount() - 1);
        content.removeView(plugins);
        content.addView(plugins, 0);
    }

    /** The enabled plugins whose masters will not load, read once per render. */
    private JSONArray missingMasters() {
        if (masterProblems == null) {
            try {
                masterProblems = ((JSONObject) ui.send("odin", "ask", ask("masters"))).getJSONArray("problems");
            } catch (IOException | JSONException | ClassCastException e) {
                masterProblems = new JSONArray();
            }
        }
        return masterProblems;
    }

    /** What one plugin is missing, as its own row says it; empty when it is not missing any. */
    private static String needs(JSONArray problems, String plugin) {
        for (int i = 0; i < problems.length(); i++) {
            JSONObject problem = problems.optJSONObject(i);
            if (problem == null || !plugin.equals(problem.optString("plugin"))) {
                continue;
            }
            JSONArray missing = problem.optJSONArray("missing");
            StringBuilder said = new StringBuilder("needs ");
            for (int j = 0; missing != null && j < missing.length(); j++) {
                said.append(j == 0 ? "" : ", ").append(missing.optString(j));
            }
            said.append(missing != null && missing.length() == 1 ? ", which is not loaded" : ", which are not loaded");
            return said.toString();
        }
        return "";
    }

    /** The line above the load order: how many plugins cannot load, and why that stops a start. */
    private static String missingSummary(JSONArray problems) {
        int count = problems.length();
        return (count == 1 ? "1 plugin names a master" : count + " plugins name masters")
            + " that is not in the load order or is turned off. The game cannot start until that is settled.";
    }

    /** The one tap: every plugin with a missing master goes off, and so do any left broken. */
    private void turnOffMissing() {
        try {
            Object answer = ui.send("odin", "ask", ask("turn_off_missing"));
            JSONArray off = ((JSONObject) answer).getJSONArray("turnedOff");
            masterProblems = null;
            ui.refresh(false, false, off.length() == 0
                ? "Nothing needed turning off."
                : "Turned off " + off.length() + (off.length() == 1 ? " plugin: " : " plugins: ") + join(off) + ".");
        } catch (IOException | JSONException | ClassCastException e) {
            ui.message(said(e, "Those plugins could not be turned off."));
        }
    }

    private static String join(JSONArray names) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < names.length(); i++) {
            out.append(i == 0 ? "" : ", ").append(names.optString(i));
        }
        return out.toString();
    }

    /**
     * A start the launcher refused. When the load order is what refused it, the player is
     * shown which plugin wants what and offered the one tap that clears it, rather than a
     * status line under the fold. Anything else is upstream's own refusal, untouched.
     */
    boolean startRefused(String error) {
        JSONArray problems = missingMasters();
        if (problems.length() == 0) {
            return false;
        }
        StringBuilder said = new StringBuilder();
        for (int i = 0; i < problems.length(); i++) {
            JSONObject problem = problems.optJSONObject(i);
            if (problem == null) {
                continue;
            }
            String plugin = problem.optString("plugin");
            said.append(plugin).append(" ").append(needs(problems, plugin)).append("\n");
        }
        new AlertDialog.Builder(activity)
            .setTitle("The load order is missing masters")
            .setMessage(said + "\nA plugin whose master is not loaded takes the game down in the load, so this start was"
                + " refused. Turning those plugins off leaves the rest of the load order as it is.")
            .setPositiveButton("Turn them off", (dialog, which) -> turnOffMissing())
            .setNegativeButton("Not now", null)
            .show();
        ui.refresh(false, false, error);
        return true;
    }

    // ---- backing saves up, and reading them back -------------------------------------

    private void backUpSaves() {
        File saves = savesDir();
        int count = SaveBackup.countSaves(saves);
        if (count == 0) {
            ui.message("There are no saves to back up yet.");
            return;
        }
        File into = SaveBackup.folderFor(downloads(), System.currentTimeMillis());
        new AlertDialog.Builder(activity)
            .setTitle("Back up saves")
            .setMessage("Copy " + count + (count == 1 ? " save" : " saves") + " to Download/"
                + SaveBackup.BACKUPS_TAIL + "/" + into.getName()
                + ". Your saves are left exactly as they are; nothing is deleted.")
            .setPositiveButton("Back up", (dialog, which) -> {
                try {
                    int copied = SaveBackup.backUp(saves, into);
                    ui.message("Backed up " + copied + (copied == 1 ? " save to Download/" : " saves to Download/")
                        + SaveBackup.BACKUPS_TAIL + "/" + into.getName() + ".");
                } catch (IOException e) {
                    ui.message(said(e, "The backup could not be written. Check that Download is writable."));
                }
            })
            .setNegativeButton("Cancel", null)
            .show();
    }

    private void restoreSaves() {
        List<File> backups = SaveBackup.backups(downloads());
        if (backups.isEmpty()) {
            ui.message("No backups in Download/" + SaveBackup.BACKUPS_TAIL + " yet.");
            return;
        }
        String[] labels = new String[backups.size()];
        for (int i = 0; i < backups.size(); i++) {
            File backup = backups.get(i);
            int count = SaveBackup.countSaves(backup);
            labels[i] = backup.getName() + "  ·  " + count + (count == 1 ? " save" : " saves");
        }
        new AlertDialog.Builder(activity)
            .setTitle("Restore saves")
            .setItems(labels, (dialog, which) -> confirmRestore(backups.get(which)))
            .setNegativeButton("Cancel", null)
            .show();
    }

    private void confirmRestore(File backup) {
        File saves = savesDir();
        int count = SaveBackup.countSaves(backup);
        int clashes = SaveBackup.clashes(backup, saves);
        String warning = clashes == 0
            ? "None of them has a name you already have."
            : clashes + (clashes == 1 ? " save of yours has" : " saves of yours have")
                + " the same name. Each one is copied to <name>.bak beside it before it is written over, so nothing is lost.";
        new AlertDialog.Builder(activity)
            .setTitle("Restore " + backup.getName())
            .setMessage("Copy " + count + (count == 1 ? " save" : " saves") + " back into this app. " + warning)
            .setPositiveButton("Restore", (dialog, which) -> {
                try {
                    int restored = SaveBackup.restore(backup, saves);
                    savesChanged();
                    ui.refresh(false, false, "Restored " + restored + (restored == 1 ? " save." : " saves."));
                } catch (IOException e) {
                    ui.message(said(e, "The restore could not finish. Your saves were not deleted."));
                }
            })
            .setNegativeButton("Cancel", null)
            .show();
    }

    // ---- the debug report ------------------------------------------------------------

    /** This build and this device, the one line of the report that is not a log. */
    String header() {
        return "NVForever " + activity.installedVersion() + " (" + activity.getPackageName() + ")"
            + "\ndevice: " + Build.MANUFACTURER + " " + Build.MODEL + " (" + Build.DEVICE + ")"
            + "\nAndroid " + Build.VERSION.RELEASE + " (API " + Build.VERSION.SDK_INT + "), "
            + (Build.SUPPORTED_ABIS.length > 0 ? Build.SUPPORTED_ABIS[0] : "unknown ABI")
            + "\nwritten: " + new Date();
    }

    private void exportLog() {
        Intent create = new Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_TITLE, reportName());
        try {
            activity.startActivityForResult(create, LauncherActivity.ODIN_EXPORT_LOG);
        } catch (android.content.ActivityNotFoundException e) {
            exportToDownload();
        }
    }

    private String reportName() {
        return "nvforever-report-" + SaveBackup.stamp(System.currentTimeMillis()) + ".txt";
    }

    /** The document picker came back. A null URI means the player left it. */
    void exportTo(Uri uri) {
        if (uri == null) {
            return;
        }
        try (OutputStream out = activity.getContentResolver().openOutputStream(uri)) {
            if (out == null) {
                throw new IOException("that file could not be opened for writing");
            }
            write(new OutputStreamWriter(out, StandardCharsets.UTF_8));
            ui.message("Debug log exported. Review it before sharing: it names your folders and mods.");
        } catch (IOException | RuntimeException e) {
            ui.message(said(e, "The debug log could not be written there."));
        }
    }

    private void exportToDownload() {
        File dir = new File(downloads(), REPORTS_TAIL);
        File file = new File(dir, reportName());
        try {
            if (!dir.isDirectory() && !dir.mkdirs()) {
                throw new IOException("Download/" + REPORTS_TAIL + " could not be made");
            }
            try (Writer out = new OutputStreamWriter(new java.io.FileOutputStream(file), StandardCharsets.UTF_8)) {
                write(out);
            }
            ui.message("Debug log written to Download/" + REPORTS_TAIL + "/" + file.getName() + ".");
        } catch (IOException e) {
            ui.message(said(e, "The debug log could not be written."));
        }
    }

    private void write(Writer out) throws IOException {
        try (Writer closing = out) {
            DebugReport.write(logsDir(), header(), closing);
        }
    }

    private void copySummary() {
        String summary;
        try {
            StringWriter text = new StringWriter();
            DebugReport.write(logsDir(), header(), text);
            String whole = text.toString();
            summary = whole.length() <= SUMMARY_CHARS
                ? whole
                : header() + "\n[earlier lines left out; Export debug log keeps them]\n"
                    + whole.substring(whole.length() - SUMMARY_CHARS);
        } catch (IOException | RuntimeException e) {
            ui.message(said(e, "The debug summary could not be read."));
            return;
        }
        ClipboardManager clipboard = (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null) {
            ui.message("This device has no clipboard service.");
            return;
        }
        clipboard.setPrimaryClip(ClipData.newPlainText("NVForever debug summary", summary));
        ui.message("Debug summary copied. Review it before sharing: it names your folders and mods.");
    }

    // ---- the gamepad -----------------------------------------------------------------

    /** Whether any gamepad or joystick is attached, which is what shows the legend row. */
    static boolean controllerPresent() {
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice device = InputDevice.getDevice(id);
            if (device == null || device.isVirtual()) {
                continue;
            }
            int sources = device.getSources();
            if ((sources & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (sources & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK) {
                return true;
            }
        }
        return false;
    }

    /**
     * A gamepad button. The D-pad and Enter are left to Android, which already moves the
     * focus and clicks with them; these are the ones nothing in a plain view hierarchy does.
     */
    boolean key(int code, KeyEvent event) {
        if (event.getAction() != KeyEvent.ACTION_DOWN || event.getRepeatCount() > 0) {
            return handled(code);
        }
        switch (code) {
            case KeyEvent.KEYCODE_BUTTON_A:
                return clickFocused();
            case KeyEvent.KEYCODE_BUTTON_B:
                return back();
            case KeyEvent.KEYCODE_BUTTON_L1:
                return turnPage(-1);
            case KeyEvent.KEYCODE_BUTTON_R1:
                return turnPage(1);
            case KeyEvent.KEYCODE_BUTTON_START:
                activity.prepareAndStart(false);
                return true;
            default:
                return false;
        }
    }

    /** The buttons {@link #key} takes, so their release is swallowed with their press. */
    private static boolean handled(int code) {
        switch (code) {
            case KeyEvent.KEYCODE_BUTTON_A:
            case KeyEvent.KEYCODE_BUTTON_B:
            case KeyEvent.KEYCODE_BUTTON_L1:
            case KeyEvent.KEYCODE_BUTTON_R1:
            case KeyEvent.KEYCODE_BUTTON_START:
                return true;
            default:
                return false;
        }
    }

    private boolean clickFocused() {
        View focused = ui.root.findFocus();
        if (focused == null || !focused.isEnabled()) {
            return false;
        }
        focused.performClick();
        return true;
    }

    /** B goes back to the first page, and off the first page it is Android's own back. */
    private boolean back() {
        if (ui.page == MobileLauncher.Page.HOME) {
            return false;
        }
        return turnPage(MobileLauncher.Page.HOME.ordinal() - ui.page.ordinal());
    }

    /** LB and RB walk upstream's nav bar, which is the order its pages are declared in. */
    private boolean turnPage(int by) {
        MobileLauncher.Page[] pages = MobileLauncher.Page.values();
        int next = Math.max(0, Math.min(pages.length - 1, ui.page.ordinal() + by));
        if (pages[next] != ui.page) {
            ui.page = pages[next];
            ui.scroll.scrollTo(0, 0);
            ui.render();
        }
        return true;
    }

    /** The left stick, which moves the focus the way the D-pad does. */
    boolean motion(MotionEvent event) {
        if ((event.getSource() & InputDevice.SOURCE_CLASS_JOYSTICK) != InputDevice.SOURCE_CLASS_JOYSTICK
            || event.getAction() != MotionEvent.ACTION_MOVE) {
            return false;
        }
        int direction = push(event.getAxisValue(MotionEvent.AXIS_X), event.getAxisValue(MotionEvent.AXIS_Y));
        if (direction == 0) {
            stickHeld = 0;
            return true;
        }
        long now = SystemClock.uptimeMillis();
        if (direction == stickHeld && now - stickMoved < STICK_REPEAT_MS) {
            return true;
        }
        stickHeld = direction;
        stickMoved = now;
        View focused = ui.root.findFocus();
        View next = focused == null ? null : focused.focusSearch(direction);
        if (next != null) {
            next.requestFocus();
        } else if (focused == null) {
            ui.root.requestFocus();
        }
        return true;
    }

    /** Which way a stick at {@code x, y} is pushed, as a {@link View} focus direction. */
    static int push(float x, float y) {
        if (!Float.isFinite(x) || !Float.isFinite(y)) {
            return 0;
        }
        if (Math.abs(y) >= Math.abs(x)) {
            if (y <= -STICK_EDGE) {
                return View.FOCUS_UP;
            }
            if (y >= STICK_EDGE) {
                return View.FOCUS_DOWN;
            }
            return 0;
        }
        if (x <= -STICK_EDGE) {
            return View.FOCUS_LEFT;
        }
        return x >= STICK_EDGE ? View.FOCUS_RIGHT : 0;
    }

    /** Remember what has the focus, because a redraw builds every view again. */
    void beforeRender() {
        View focused = ui.root == null ? null : ui.root.findFocus();
        focusOn = focused instanceof TextView ? ((TextView) focused).getText().toString() : null;
        compact = measure();
        columns = null;
        columnNext = 0;
        masterProblems = null;
        // Home and Settings are lists of cards, so compact they are dealt into columns and
        // the page that was four rows long becomes one screen. The Mods page is a load order:
        // it keeps its own column, and its rows are slim instead (pluginRows).
        boolean dealt = ui.page == MobileLauncher.Page.HOME || ui.page == MobileLauncher.Page.SETTINGS;
        columnCount = dealt ? compact.columns(widthDp()) : 1;
    }

    /** Put the focus back on the row it was on, or on the first row of a new page. */
    void afterRender() {
        if (ui.root == null) {
            return;
        }
        ui.root.post(() -> {
            if (ui.root.isInTouchMode()) {
                return;
            }
            if (focusOn != null && refocus(ui.root, focusOn)) {
                return;
            }
            if (ui.root.findFocus() == null) {
                ui.root.requestFocus();
            }
        });
    }

    private static boolean refocus(View view, String label) {
        if (view instanceof TextView && view.isFocusable() && view.isEnabled()
            && label.contentEquals(((TextView) view).getText())) {
            return view.requestFocus();
        }
        if (view instanceof ViewGroup) {
            ViewGroup group = (ViewGroup) view;
            for (int i = 0; i < group.getChildCount(); i++) {
                if (refocus(group.getChildAt(i), label)) {
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * The focus highlight, in upstream's own theme: the gold the launcher accents everything
     * else with, as a ring round the row that has the focus. Not focusable in touch mode, so a
     * tap leaves nothing behind.
     */
    static void focusHighlight(View view, GradientDrawable fill, int gold, int ring) {
        view.setFocusableInTouchMode(false);
        if (Build.VERSION.SDK_INT >= 26) {
            view.setDefaultFocusHighlightEnabled(false);
        }
        view.setOnFocusChangeListener((v, focused) -> {
            fill.setStroke(focused ? ring : 0, gold);
            if (v instanceof TextView) {
                ((TextView) v).setTypeface(Typeface.DEFAULT, focused ? Typeface.BOLD : Typeface.NORMAL);
            }
        });
    }

    // ---- small shared things ---------------------------------------------------------

    /** One {@code odin} ask, as upstream's request shape carries it. */
    private static JSONObject ask(String name, String... pairs) throws JSONException {
        JSONObject object = new JSONObject();
        object.put("ask", name);
        for (int i = 0; i + 1 < pairs.length; i += 2) {
            object.put(pairs[i], pairs[i + 1]);
        }
        return object;
    }

    /** What to show the player: the error's own words where it has them. */
    private static String said(Exception e, String fallback) {
        String message = e.getMessage();
        return message == null || message.isEmpty() ? fallback : message;
    }
}
