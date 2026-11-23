package org.nvforever.android;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.Settings;
import android.text.TextUtils;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The screen the app opens on: the launcher, design v2 direction A.
 *
 * <p>Home is a main menu: a short left-aligned vertical list (Continue, Load save, Play, Data,
 * Settings, About) with one warm focus bar, and a detail panel on the right that follows the
 * focused item. Setup runs first on a fresh install; Settings and Advanced are one screen each,
 * laid out in three columns; a slim status strip and a controller legend end every screen.
 *
 * <p>What it writes, and nothing else: {@code files/profiles.txt}, {@code files/data-path.txt}
 * and {@code files/profile.txt} (the engine reads the last two, nvf-android/src/paths.rs),
 * {@code files/android-continue.txt} for one CONTINUE or one chosen save, the shared
 * {@code NVForever/nvforever-controls.txt}, and, only when the player asks for it,
 * {@code Download/NVForever-backups/} ({@link SaveBackup}). It never writes into a Data folder,
 * never moves a save, and never touches a plugin list.
 *
 * <p>Everything is reachable with a D-pad and A and B alone: every control is focusable,
 * A activates, B goes one level out, and the shoulder buttons page through the sections.
 */
public class LauncherActivity extends Activity {
    private static final int REQUEST_LEGACY_PERMISSION = 1;
    private static final int REQUEST_TREE = 2;
    private static final int REQUEST_EXPORT_LOG = 3;
    private static final int REQUEST_NEW_PROFILE_TREE = 4;
    private static final int REQUEST_DRIVER_ZIP = 5;

    private static final String PREFS = "launcher";
    private static final String PREF_DEBUG_UNLOCKED = "debug-unlocked";
    private static final String PREF_DEBUG_CONFIRMED = "debug-confirmed";
    private static final String PREF_KEEP_AWAKE = "keep-awake";
    private static final String PREF_SETUP_DONE = "setup-done";

    /** The launcher's screens. Setup, Load save and Advanced are not in the shoulder round. */
    private enum Section { SETUP, HOME, SAVES, DATA, SETTINGS, ABOUT, DEBUG }

    private static final Section[] PAGED = {Section.HOME, Section.DATA, Section.SETTINGS, Section.ABOUT};

    /** The main menu, in the order it is drawn. */
    private enum Menu { CONTINUE, LOAD, PLAY, DATA, SETTINGS, ABOUT }

    private SharedPreferences prefs;
    private Profiles profiles;
    private Map<String, Boolean> controls = new LinkedHashMap<>();
    private DataCheck.Result check;
    private Section section = Section.HOME;
    /** The name of the profile being created, between naming it and picking its folder. */
    private String pendingProfileName;
    /** Home's right-hand panel, refilled whenever the focused menu item changes. */
    private LinearLayout detail;

    // ---- lifecycle ---------------------------------------------------------------------

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        reload();
        section = setupNeeded() ? Section.SETUP : Section.HOME;
        show();
    }

    @Override
    protected void onResume() {
        super.onResume();
        reload();
        if (setupNeeded()) {
            section = Section.SETUP;
        } else if (section == Section.SETUP && prefs.getBoolean(PREF_SETUP_DONE, false)) {
            section = Section.HOME;
        }
        show();
        resumeRequested(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        resumeRequested(intent);
    }

    /** Everything the screens are drawn from, read once a resume. */
    private void reload() {
        profiles = Profiles.load(getFilesDir(), defaultDataDir());
        controls = ControlsFile.read(controlsFile());
        check = DataCheck.scan(profiles.active().dataDir());
    }

    private boolean setupNeeded() {
        return !hasFilesAccess() || !check.ready();
    }

    // ---- the frame every section sits in -----------------------------------------------

    private void show() {
        LinearLayout root = Ui.column(this);
        root.setBackground(Ui.background(this));
        int side = Ui.dp(this, Ui.GUTTER_DP);
        root.setPaddingRelative(side, Ui.dp(this, 10), side, Ui.dp(this, 12));

        LinearLayout content = Ui.column(this);
        content.addView(buildSection(), new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        root.addView(content, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        root.addView(statusStrip(), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        root.addView(legend(), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 4, 0));
        setContentView(Ui.page(this, root));
        View focus = root.findFocus();
        if (focus == null) {
            root.post(() -> {
                View first = defaultFocus;
                if (first != null && first.isShown()) {
                    first.requestFocus();
                } else {
                    root.requestFocus(View.FOCUS_DOWN);
                }
            });
        }
    }

    /** The control each screen opens with, so a controller player can press A straight away. */
    private View defaultFocus;

    /** The name and the one-line "what this screen is for", above every screen but Home. */
    private View header(CharSequence title, CharSequence crumb, CharSequence back) {
        LinearLayout bar = Ui.row(this);
        bar.setGravity(Gravity.BOTTOM);
        bar.addView(Ui.screenTitle(this, title));
        TextView note = Ui.crumb(this, crumb);
        note.setSingleLine(true);
        note.setEllipsize(TextUtils.TruncateAt.END);
        LinearLayout.LayoutParams np = new LinearLayout.LayoutParams(
                0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        np.setMarginStart(Ui.dp(this, 12));
        np.bottomMargin = Ui.dp(this, 2);
        bar.addView(note, np);
        if (back != null) {
            TextView right = Ui.crumb(this, back);
            right.setGravity(Gravity.END);
            bar.addView(right);
        }
        return bar;
    }

    private String buildChipText() {
        return BuildConfig.VERSION_NAME + "  " + Build.SUPPORTED_ABIS[0] + "  " + Build.MODEL;
    }

    /** The facts at the foot of the screen, one line, nothing wrapped. */
    private View statusStrip() {
        LinearLayout strip = Ui.statusStrip(this);
        Profiles.Profile active = profiles.active();
        Ui.cell(strip, this, getString(R.string.strip_profile), active.name, false, Ui.INK);
        if (section == Section.DEBUG) {
            Ui.cell(strip, this, getString(R.string.strip_logs),
                    getResources().getQuantityString(R.plurals.count_runs, runCount(), runCount()),
                    true, Ui.INK_2);
            Ui.cell(strip, this, getString(R.string.strip_last_run), lastRunText(), false, Ui.INK);
            Ui.cell(strip, this, getString(R.string.strip_driver), driverLabel(), false, Ui.INK);
        } else {
            Ui.cell(strip, this, getString(R.string.strip_data), dataVerdict(),
                    false, check.ready() ? Ui.INK : Ui.BAD);
            Ui.cell(strip, this, getString(R.string.strip_addons), addonsLine(check), false, Ui.INK);
            Ui.cell(strip, this, getString(R.string.strip_driver), driverLabel(), false, Ui.INK);
            int testsOn = ControlsFile.testsOn(controls).size();
            if (testsOn > 0) {
                Ui.cell(strip, this, getString(R.string.strip_tests),
                        getString(R.string.home_tests_on_chip, testsOn), false, Ui.WARN);
            }
        }
        View push = new View(this);
        strip.addView(push, new LinearLayout.LayoutParams(0, 1, 1f));
        strip.addView(buildChip());
        return strip;
    }

    /** The version, and the long press that shows Advanced. */
    private View buildChip() {
        TextView build = Ui.micro(this, buildChipText(), Ui.INK_2);
        build.setPaddingRelative(Ui.dp(this, 8), Ui.dp(this, 4), Ui.dp(this, 4), Ui.dp(this, 4));
        build.setFocusable(true);
        build.setOnClickListener(v -> {});
        build.setOnLongClickListener(v -> {
            unlockDebug();
            go(Section.DEBUG);
            return true;
        });
        build.setContentDescription(buildChipText() + ". " + getString(R.string.legend_hold_version));
        return build;
    }

    /** The controller legend, which says what A and B do on the screen you are on. */
    private View legend() {
        LinearLayout bar = Ui.legend(this);
        switch (section) {
            case HOME:
                Ui.legendItem(bar, this, "A", getString(R.string.legend_select));
                Ui.legendItem(bar, this, "B", getString(R.string.legend_back));
                Ui.legendItem(bar, this, "Start", getString(R.string.legend_play));
                Ui.legendItem(bar, this, "Y", getString(R.string.section_settings));
                Ui.legendHint(bar, this, getString(R.string.legend_hold_version));
                break;
            case SETTINGS:
                Ui.legendItem(bar, this, "A", getString(R.string.legend_change));
                Ui.legendItem(bar, this, "B", getString(R.string.legend_back));
                Ui.legendItem(bar, this, "LB RB", getString(R.string.legend_section));
                Ui.legendHint(bar, this, getString(R.string.legend_applies_next_start));
                break;
            case DEBUG:
                Ui.legendItem(bar, this, "A", getString(R.string.legend_toggle));
                Ui.legendItem(bar, this, "B", getString(R.string.legend_back));
                Ui.legendItem(bar, this, "LB RB", getString(R.string.legend_section));
                Ui.legendHint(bar, this, getString(R.string.legend_advanced_hidden));
                break;
            default:
                Ui.legendItem(bar, this, "A", getString(R.string.legend_select));
                Ui.legendItem(bar, this, "B", getString(R.string.legend_back));
                Ui.legendItem(bar, this, "LB RB", getString(R.string.legend_section));
                break;
        }
        return bar;
    }

    private void go(Section s) {
        section = s;
        defaultFocus = null;
        show();
    }

    private View buildSection() {
        defaultFocus = null;
        detail = null;
        switch (section) {
            case SETUP: return setupScreen();
            case SAVES: return savesScreen();
            case DATA: return dataScreen();
            case SETTINGS: return settingsScreen();
            case DEBUG: return debugScreen();
            case ABOUT: return aboutScreen();
            default: return homeScreen();
        }
    }

    // ---- controller keys ---------------------------------------------------------------

    @Override
    public boolean onKeyDown(int code, KeyEvent event) {
        switch (code) {
            case KeyEvent.KEYCODE_BUTTON_L1:
            case KeyEvent.KEYCODE_BUTTON_R1:
                // From Setup too: a player whose folder went missing has to be able to reach
                // the Data area and pick another profile.
                page(code == KeyEvent.KEYCODE_BUTTON_R1 ? 1 : -1);
                return true;
            case KeyEvent.KEYCODE_BUTTON_B:
                back();
                return true;
            case KeyEvent.KEYCODE_BUTTON_X:
                if (section == Section.HOME) {
                    go(Section.DATA);
                    return true;
                }
                return super.onKeyDown(code, event);
            case KeyEvent.KEYCODE_BUTTON_Y:
                if (section == Section.HOME) {
                    go(Section.SETTINGS);
                    return true;
                }
                return super.onKeyDown(code, event);
            case KeyEvent.KEYCODE_BUTTON_START:
                if (section == Section.HOME && check.ready()) {
                    startGame(null);
                    return true;
                }
                return super.onKeyDown(code, event);
            default:
                return super.onKeyDown(code, event);
        }
    }

    @Override
    public void onBackPressed() {
        back();
    }

    /** B, and the system's back: one level out, never out of the app. */
    private void back() {
        if (section == Section.DEBUG) {
            go(Section.SETTINGS);
        } else if (section != Section.HOME && section != Section.SETUP) {
            go(Section.HOME);
        }
    }

    private void page(int step) {
        List<Section> round = new ArrayList<>();
        for (Section s : PAGED) {
            round.add(s);
        }
        if (prefs.getBoolean(PREF_DEBUG_UNLOCKED, false)) {
            round.add(Section.DEBUG);
        }
        int at = Math.max(0, round.indexOf(section));
        go(round.get((at + step + round.size()) % round.size()));
    }

    // ---- Home: the main menu -----------------------------------------------------------

    private View homeScreen() {
        LinearLayout screen = Ui.column(this);

        LinearLayout names = Ui.column(this);
        TextView wordmark = Ui.label(this, wordmarkText(), Ui.MENU_SP, Ui.INK, true);
        names.addView(wordmark);
        TextView tagline = Ui.crumb(this, getString(R.string.app_tagline));
        tagline.setMaxLines(1);
        tagline.setEllipsize(TextUtils.TruncateAt.END);
        names.addView(tagline);
        screen.addView(names, Ui.wide(this, 10));

        // The two things a player must be told without asking: the last run died, and the
        // graphics driver they chose was not the one the last start managed to use.
        if (RunState.lastRun(logsDir()) == RunState.LastRun.CRASHED) {
            screen.addView(Ui.band(this, getString(R.string.home_last_run_crash_action), Ui.WARN),
                    Ui.wide(this, 8));
        }
        String fellBack = GpuDrivers.lastStartFellBack(this);
        if (fellBack != null) {
            screen.addView(Ui.band(this, getString(R.string.home_driver_fell_back, fellBack), Ui.WARN),
                    Ui.wide(this, 8));
        }

        LinearLayout main = Ui.row(this);
        main.setGravity(Gravity.TOP);

        LinearLayout menu = Ui.column(this);
        menu.setGravity(Gravity.CENTER_VERTICAL);
        RunState.Save newest = RunState.newestSave(profiles.active().savesDir(getFilesDir()));
        int saves = RunState.countSaves(profiles.active().savesDir(getFilesDir()));
        if (newest != null) {
            View item = menuItem(Menu.CONTINUE, getString(R.string.menu_continue), null, true,
                    v -> startGame(newest.file));
            menu.addView(item, Ui.wide(this, 0));
            defaultFocus = item;
            menu.addView(menuItem(Menu.LOAD, getString(R.string.menu_load),
                    getResources().getQuantityString(R.plurals.count_saves_short, saves, saves),
                    false, v -> go(Section.SAVES)), Ui.wide(this, 0));
        }
        View play = menuItem(Menu.PLAY, getString(R.string.menu_play), null, newest == null,
                v -> startGame(null));
        menu.addView(play, Ui.wide(this, 0));
        if (defaultFocus == null) {
            defaultFocus = play;
        }
        menu.addView(menuItem(Menu.DATA, getString(R.string.menu_data), null, false,
                v -> go(Section.DATA)), Ui.wide(this, 0));
        menu.addView(menuItem(Menu.SETTINGS, getString(R.string.menu_settings), null, false,
                v -> go(Section.SETTINGS)), Ui.wide(this, 0));
        menu.addView(menuItem(Menu.ABOUT, getString(R.string.menu_about), null, false,
                v -> go(Section.ABOUT)), Ui.wide(this, 0));
        main.addView(menu, new LinearLayout.LayoutParams(Ui.dp(this, 330),
                ViewGroup.LayoutParams.MATCH_PARENT));

        LinearLayout right = Ui.row(this);
        right.setGravity(Gravity.TOP);
        right.addView(Ui.hairline(this), new LinearLayout.LayoutParams(1,
                ViewGroup.LayoutParams.MATCH_PARENT));
        LinearLayout pane = Ui.column(this);
        pane.setPaddingRelative(Ui.dp(this, 30), Ui.dp(this, 4), 0, 0);
        detail = Ui.column(this);
        pane.addView(detail, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        LinearLayout quick = Ui.group(this, getString(R.string.home_quick));
        LinearLayout quickList = Ui.list(this);
        quickList.addView(Ui.switchRow(this, getString(R.string.home_toggle_touch), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.TOUCH)), true, Ui.ROW_DP,
                (b, on) -> setControl(ControlsFile.TOUCH, on)));
        quickList.addView(Ui.switchRow(this, getString(R.string.home_toggle_controller), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.CONTROLLER)), true, Ui.ROW_DP,
                (b, on) -> setControl(ControlsFile.CONTROLLER, on)));
        quick.addView(quickList);
        pane.addView(quick, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 12, 0));
        right.addView(pane, new LinearLayout.LayoutParams(0,
                ViewGroup.LayoutParams.MATCH_PARENT, 1f));
        main.addView(right, new LinearLayout.LayoutParams(0,
                ViewGroup.LayoutParams.MATCH_PARENT, 1f));

        screen.addView(main, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        showDetail(newest != null ? Menu.CONTINUE : Menu.PLAY);
        return screen;
    }

    /** NVForever, with the second half in the accent. One wordmark, no letter spacing. */
    private CharSequence wordmarkText() {
        String name = getString(R.string.app_name);
        android.text.SpannableString text = new android.text.SpannableString(name);
        int from = Math.min(2, name.length());
        text.setSpan(new android.text.style.ForegroundColorSpan(Ui.AMBER), from, name.length(),
                android.text.Spanned.SPAN_INCLUSIVE_EXCLUSIVE);
        return text;
    }

    /** A menu row that also drives the detail panel: focus shows it, A does it. */
    private View menuItem(Menu which, String title, String hint, boolean hero, View.OnClickListener action) {
        LinearLayout item = Ui.menuItem(this, title, hint, hero, action);
        item.setOnFocusChangeListener((v, has) -> {
            if (has) {
                showDetail(which);
            }
        });
        return item;
    }

    /** The right-hand panel: what the focused menu item leads to, and nothing clickable. */
    private void showDetail(Menu which) {
        if (detail == null) {
            return;
        }
        detail.removeAllViews();
        Profiles.Profile active = profiles.active();
        switch (which) {
            case CONTINUE: {
                RunState.Save newest = RunState.newestSave(active.savesDir(getFilesDir()));
                detailHead(getString(R.string.menu_continue));
                if (newest == null) {
                    detailNote(getString(R.string.home_no_save));
                    break;
                }
                detailThumbnail();
                detailTitle(newest.name());
                detailMeta(getString(R.string.home_detail_save_meta, ago(newest.modified), active.name));
                detailPath(newest.file.getName() + "   " + size(newest.file.length()));
                break;
            }
            case LOAD: {
                int saves = RunState.countSaves(active.savesDir(getFilesDir()));
                detailHead(getString(R.string.menu_load));
                detailTitle(getResources().getQuantityString(R.plurals.count_saves, saves, saves));
                detailMeta(getString(R.string.saves_crumb));
                List<File> newestFew = SaveBackup.saves(active.savesDir(getFilesDir()));
                sortNewestFirst(newestFew);
                for (int i = 0; i < Math.min(3, newestFew.size()); i++) {
                    detailPath(newestFew.get(i).getName());
                }
                break;
            }
            case PLAY:
                detailHead(getString(R.string.menu_play));
                detailTitle(getString(R.string.home_play_sub));
                detailMeta(getString(R.string.home_detail_play_meta, active.name));
                detailPath(active.data);
                break;
            case DATA:
                detailHead(getString(R.string.menu_data));
                detailTitle(active.name);
                detailMeta(dataVerdict());
                detailMeta(getString(R.string.strip_addons) + ": " + addonsLine(check));
                detailPath(active.data);
                break;
            case SETTINGS:
                detailHead(getString(R.string.menu_settings));
                detailTitle(getString(R.string.settings_detail_title));
                detailMeta(getString(R.string.settings_note));
                detailPath(getString(R.string.strip_driver) + " " + driverLabel());
                break;
            default:
                detailHead(getString(R.string.menu_about));
                detailTitle(getString(R.string.app_name));
                detailMeta(getString(R.string.about_detail_meta));
                detailPath(buildChipText());
                break;
        }
    }

    private void detailHead(String text) {
        detail.addView(Ui.groupHead(this, text), Ui.wide(this, 10));
    }

    private void detailTitle(String text) {
        TextView t = Ui.label(this, text, Ui.LEAD_SP, Ui.INK, true);
        t.setSingleLine(true);
        t.setEllipsize(TextUtils.TruncateAt.END);
        detail.addView(t);
    }

    private void detailMeta(String text) {
        TextView t = Ui.label(this, text, Ui.SMALL_SP, Ui.INK_2, false);
        t.setMaxLines(3);
        t.setEllipsize(TextUtils.TruncateAt.END);
        detail.addView(t, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 4, 0));
    }

    private void detailNote(String text) {
        detail.addView(Ui.note(this, text), Ui.wide(this, 0));
    }

    private void detailPath(String text) {
        TextView t = Ui.micro(this, text, Ui.INK_3);
        t.setSingleLine(true);
        t.setEllipsize(TextUtils.TruncateAt.MIDDLE);
        detail.addView(t, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
    }

    /**
     * The frame where a save's picture would be. NVForever's saves hold no thumbnail, so the
     * frame says so rather than showing a picture of anything.
     */
    private void detailThumbnail() {
        LinearLayout box = Ui.row(this);
        box.setGravity(Gravity.CENTER);
        android.graphics.drawable.GradientDrawable frame = new android.graphics.drawable.GradientDrawable();
        frame.setColor(Ui.BG_2);
        frame.setStroke(1, Ui.RULE_2);
        box.setBackground(frame);
        box.addView(Ui.micro(this, getString(R.string.home_no_thumbnail), 0xFF4A443B));
        detail.addView(box, Ui.params(this, Ui.dp(this, 200), Ui.dp(this, 112), 0, 0, 10));
    }

    // ---- the one-line facts the strip and the detail share -------------------------------

    private String driverLabel() {
        String mode = GpuDrivers.mode(this);
        if (GpuDrivers.AUTOMATIC.equals(mode)) {
            return getString(R.string.settings_driver_automatic);
        }
        if (mode == null) {
            return getString(R.string.settings_driver_system);
        }
        for (String[] driver : GpuDrivers.installed(this)) {
            if (driver[0].equals(mode)) {
                return driver[1];
            }
        }
        return getString(R.string.settings_driver_system);
    }

    /** The data folder in one phrase: ready, or the first thing that is wrong with it. */
    private String dataVerdict() {
        if (check.closed) {
            return Ui.mark(false) + " " + getString(R.string.status_data_closed);
        }
        if (!check.exists) {
            return Ui.mark(false) + " " + getString(R.string.check_folder_unreadable);
        }
        if (!check.baseMasterFound()) {
            return Ui.mark(false) + " " + getString(R.string.check_base_missing);
        }
        if (check.archivesFound() != check.archivesTotal()) {
            return Ui.mark(false) + " "
                    + getString(R.string.check_archives, check.archivesFound(), check.archivesTotal());
        }
        return Ui.mark(true) + " " + getString(R.string.status_data_ok);
    }

    /**
     * The add-ons, said the way round that is true: none is the normal state of a plain copy
     * of the game, and no add-on is required, so it carries no mark.
     */
    private String addonsLine(DataCheck.Result scan) {
        int found = scan.addonsFound();
        return found == 0 ? getString(R.string.check_addons_none)
                : getResources().getQuantityString(R.plurals.check_addons_installed, found, found);
    }

    private int runCount() {
        File[] logs = logsDir().listFiles(
                (directory, name) -> name.startsWith("nvforever-") && name.endsWith(".log"));
        return logs == null ? 0 : logs.length;
    }

    private String lastRunText() {
        RunState.LastRun last = RunState.lastRun(logsDir());
        return getString(last == RunState.LastRun.NONE ? R.string.home_last_run_none
                : last == RunState.LastRun.OK ? R.string.home_last_run_ok
                : R.string.home_last_run_crash);
    }

    /** How long ago, in the player's own language, with the right plural. */
    private String ago(long when) {
        long minutes = Math.max(0, (System.currentTimeMillis() - when) / 60000L);
        if (minutes < 1) {
            return getString(R.string.time_just_now);
        }
        if (minutes < 60) {
            return getResources().getQuantityString(R.plurals.time_minutes_ago, (int) minutes, (int) minutes);
        }
        long hours = minutes / 60;
        if (hours < 24) {
            return getResources().getQuantityString(R.plurals.time_hours_ago, (int) hours, (int) hours);
        }
        long days = hours / 24;
        return getResources().getQuantityString(R.plurals.time_days_ago, (int) days, (int) days);
    }

    /** A file size, in the units the system formats for this language. */
    private String size(long bytes) {
        return android.text.format.Formatter.formatShortFileSize(this, bytes);
    }

    // ---- Load save ------------------------------------------------------------------------

    /**
     * Every save of the active profile, newest first, read only: a name, when it was written
     * and how big it is. Choosing one starts the game loading it, through the same one-start
     * request CONTINUE uses (files/android-continue.txt, paths::continue_save).
     */
    private View savesScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.saves_title), getString(R.string.saves_crumb),
                getString(R.string.legend_back_menu)), Ui.wide(this, 10));
        Profiles.Profile active = profiles.active();
        List<File> saves = SaveBackup.saves(active.savesDir(getFilesDir()));
        if (saves.isEmpty()) {
            screen.addView(Ui.note(this, getString(R.string.saves_none)));
            return screen;
        }
        sortNewestFirst(saves);
        LinearLayout list = Ui.list(this);
        for (final File save : saves) {
            View row = Ui.valueRow(this, saveName(save),
                    getString(R.string.saves_row_meta, ago(save.lastModified()), size(save.length())),
                    save.getName(), true, Ui.ROW_DP, v -> startGame(save));
            list.addView(row);
            if (defaultFocus == null) {
                defaultFocus = row;
            }
        }
        screen.addView(list);
        return screen;
    }

    private static String saveName(File save) {
        String name = save.getName();
        int dot = name.lastIndexOf('.');
        return dot > 0 ? name.substring(0, dot) : name;
    }

    private static void sortNewestFirst(List<File> files) {
        java.util.Collections.sort(files, (a, b) -> Long.compare(b.lastModified(), a.lastModified()));
    }

    // ---- Setup -------------------------------------------------------------------------

    private View setupScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.setup_title), getString(R.string.setup_intro), null),
                Ui.wide(this, 12));

        LinearLayout columns = Ui.row(this);
        columns.setGravity(Gravity.TOP);

        boolean access = hasFilesAccess();
        LinearLayout step1 = Ui.group(this, getString(R.string.setup_step_access_title));
        step1.addView(Ui.note(this, getString(R.string.setup_step_access_why)));
        if (Build.VERSION.SDK_INT < 30) {
            step1.addView(Ui.note(this, getString(R.string.setup_step_access_legacy)));
        }
        LinearLayout step1List = Ui.list(this);
        if (access) {
            step1List.addView(Ui.valueRow(this, getString(R.string.setup_step_access_granted), null,
                    Ui.mark(true), true, Ui.ROW_DP, null));
        } else {
            View grant = Ui.chevronRow(this, getString(R.string.setup_step_access_grant),
                    getString(R.string.setup_step_access_return), Ui.ROW_DP, v -> requestFilesAccess());
            step1List.addView(grant);
            defaultFocus = grant;
        }
        step1.addView(step1List, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        columns.addView(step1, columnParams(0));

        LinearLayout step2 = Ui.group(this, getString(R.string.setup_step_folder_title));
        step2.addView(Ui.note(this, getString(R.string.setup_step_folder_why)));
        LinearLayout step2List = Ui.list(this);
        View choose = Ui.pathRow(this, getString(R.string.setup_step_folder_choose),
                profiles.active().data, Ui.ROW_DP, v -> pickFolder(REQUEST_TREE));
        choose.setEnabled(access);
        step2List.addView(choose);
        View create = Ui.chevronRow(this, getString(R.string.setup_step_folder_create),
                getString(R.string.setup_step_folder_create_note), Ui.ROW_DP, v -> createDefaultFolder());
        create.setEnabled(access);
        step2List.addView(create);
        step2.addView(step2List, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        step2.addView(Ui.note(this, getString(R.string.setup_step_folder_profile_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        if (access && defaultFocus == null) {
            defaultFocus = choose;
        }
        columns.addView(step2, columnParams(16));

        LinearLayout step3 = Ui.group(this, getString(R.string.setup_step_check_title));
        step3.addView(checkList());
        LinearLayout step3List = Ui.list(this);
        if (check.ready()) {
            View play = Ui.chevronRow(this, getString(R.string.menu_play),
                    getString(R.string.setup_step_check_ready), Ui.ROW_DP, v -> {
                        prefs.edit().putBoolean(PREF_SETUP_DONE, true).apply();
                        startGame(null);
                    });
            step3List.addView(play);
            defaultFocus = play;
        } else {
            step3List.addView(Ui.valueRow(this, getString(R.string.common_verify),
                    getString(R.string.setup_step_check_blocked), Ui.mark(false), true, Ui.ROW_DP, v -> {
                        reload();
                        show();
                    }));
            // The escape hatch stays, demoted: it is useful, and it is not the happy path.
            View anyway = Ui.chevronRow(this, getString(R.string.setup_start_anyway), null,
                    Ui.ROW_DENSE_DP, v -> startGame(null));
            anyway.setEnabled(access && !check.closed);
            step3List.addView(anyway);
        }
        step3.addView(step3List, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        step3.addView(Ui.note(this, getString(R.string.setup_step_check_why)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        columns.addView(step3, columnParams(16));
        screen.addView(columns, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    /** One of the three columns Setup, Settings and Advanced are laid out in. */
    private LinearLayout.LayoutParams columnParams(int startDp) {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(0,
                ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        p.setMarginStart(Ui.dp(this, startDp));
        return p;
    }

    /** The tick list of required files and detected add-ons, with sizes. */
    private View checkList() {
        LinearLayout list = Ui.column(this);
        if (check.closed) {
            list.addView(Ui.label(this, getString(R.string.setup_blocked_android_data),
                    Ui.SMALL_SP, Ui.BAD, false));
            return list;
        }
        if (!check.exists) {
            list.addView(Ui.label(this, getString(R.string.setup_blocked_missing_folder),
                    Ui.SMALL_SP, Ui.BAD, false));
            return list;
        }
        for (DataCheck.Entry e : check.required) {
            list.addView(Ui.micro(this, Ui.mark(e.present) + " " + e.name + "   "
                    + (e.present ? size(e.size) : getString(R.string.check_missing_file, e.name)),
                    e.present ? Ui.INK_2 : Ui.BAD));
        }
        list.addView(Ui.micro(this, getString(R.string.strip_addons) + " " + addonsLine(check)
                + (check.addonsFound() == 0 ? "" : ": " + addonNames()), Ui.INK_3));
        return list;
    }

    private String addonNames() {
        List<String> found = new ArrayList<>();
        for (DataCheck.Entry e : check.addons) {
            if (e.present) {
                found.add(e.name);
            }
        }
        return found.isEmpty() ? getString(R.string.common_none) : TextUtils.join(", ", found);
    }

    // ---- Data --------------------------------------------------------------------------

    private View dataScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.data_title), getString(R.string.data_crumb),
                getString(R.string.legend_back_menu)), Ui.wide(this, 10));

        LinearLayout columns = Ui.row(this);
        columns.setGravity(Gravity.TOP);

        LinearLayout list = Ui.group(this, getString(R.string.data_profiles_title));
        LinearLayout rows = Ui.list(this);
        for (Profiles.Profile p : profiles.all()) {
            rows.addView(profileRow(p));
        }
        rows.addView(Ui.chevronRow(this, getString(R.string.data_profiles_new), null,
                Ui.ROW_DP, v -> newProfile()));
        list.addView(rows);
        list.addView(Ui.note(this, getString(R.string.data_profiles_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        columns.addView(list, columnParams(0));

        LinearLayout right = Ui.column(this);
        LinearLayout saves = Ui.group(this, getString(R.string.settings_group_saves));
        saves.addView(savesRows());
        saves.addView(Ui.note(this, getString(R.string.saves_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        right.addView(saves, Ui.wide(this, 16));

        // Phase 2 lives here. The row is shown, not hidden, so the shape of the area is honest.
        LinearLayout soon = Ui.group(this, getString(R.string.data_mods_soon));
        soon.addView(Ui.note(this, getString(R.string.data_mods_soon_note)));
        right.addView(soon, Ui.wide(this, 0));
        columns.addView(right, columnParams(16));

        screen.addView(columns, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View profileRow(Profiles.Profile p) {
        boolean active = p.id.equals(profiles.active().id);
        DataCheck.Result scan = active ? check : DataCheck.scan(p.dataDir());
        String badge = !scan.exists ? getString(R.string.data_profiles_unavailable)
                : Ui.mark(scan.ready()) + " " + getString(R.string.check_archives,
                        scan.archivesFound(), scan.archivesTotal());
        TextView value = Ui.micro(this, badge, scan.exists && scan.ready() ? Ui.INK_2 : Ui.BAD);
        LinearLayout row = Ui.row(this, p.name + (active ? "   " + getString(R.string.data_profiles_active) : ""),
                p.data, true, null, value, Ui.ROW_DP, true, v -> profileActions(p, scan));
        row.setContentDescription(p.name + ". " + p.data + ". " + badge);
        if (active) {
            defaultFocus = row;
        }
        return row;
    }

    private void profileActions(Profiles.Profile p, DataCheck.Result scan) {
        List<String> items = new ArrayList<>();
        List<Runnable> actions = new ArrayList<>();
        if (!p.id.equals(profiles.active().id)) {
            items.add(getString(R.string.data_profiles_use));
            actions.add(() -> {
                if (!scan.ready()) {
                    toast(getString(R.string.data_profiles_cannot_use));
                    return;
                }
                profiles.setActive(p.id);
                saveProfiles();
                reload();
                show();
            });
        }
        items.add(getString(R.string.common_change_folder));
        actions.add(() -> {
            pendingProfileName = null;
            profiles.setActive(p.id);
            saveProfiles();
            pickFolder(REQUEST_TREE);
        });
        items.add(getString(R.string.data_profiles_rename));
        actions.add(() -> askForName(p.name, name -> {
            profiles.rename(p.id, name);
            saveProfiles();
            reload();
            show();
        }));
        if (!Profiles.DEFAULT_ID.equals(p.id) && profiles.size() > 1) {
            items.add(getString(R.string.data_profiles_forget));
            actions.add(() -> confirm(getString(R.string.data_profiles_forget_confirm), () -> {
                profiles.forget(p.id);
                saveProfiles();
                reload();
                show();
            }));
        }
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setTitle(p.name)
                .setItems(items.toArray(new String[0]), (d, which) -> actions.get(which).run())
                .setNegativeButton(R.string.common_cancel, null)
                .create();
        dialog.show();
    }

    private void newProfile() {
        askForName("", name -> {
            pendingProfileName = name;
            pickFolder(REQUEST_NEW_PROFILE_TREE);
        });
    }

    /** One short text field, for a profile's name. The only typed input in the launcher. */
    private void askForName(String current, java.util.function.Consumer<String> then) {
        EditText field = new EditText(this);
        field.setText(current);
        field.setSingleLine(true);
        field.setTextColor(Ui.INK);
        new AlertDialog.Builder(this)
                .setTitle(R.string.data_profiles_new_name)
                .setView(field)
                .setPositiveButton(R.string.common_ok, (d, w) -> {
                    String name = field.getText().toString().trim();
                    if (!name.isEmpty()) {
                        then.accept(name);
                    }
                })
                .setNegativeButton(R.string.common_cancel, null)
                .show();
    }

    /** A confirm whose focus starts on Cancel, because every one of them is destructive. */
    private void confirm(String message, Runnable yes) {
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setMessage(message)
                .setPositiveButton(R.string.common_yes, (d, w) -> yes.run())
                .setNegativeButton(R.string.common_cancel, null)
                .create();
        dialog.show();
        dialog.getButton(AlertDialog.BUTTON_NEGATIVE).requestFocus();
    }

    private void saveProfiles() {
        try {
            profiles.save(getFilesDir());
        } catch (IOException e) {
            toast(getString(R.string.error_save_choice, e.toString()));
        }
    }

    // ---- Settings ----------------------------------------------------------------------

    private View settingsScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.settings_title), getString(R.string.settings_note),
                getString(R.string.legend_back_menu)), Ui.wide(this, 10));

        LinearLayout columns = Ui.row(this);
        columns.setGravity(Gravity.TOP);
        columns.addView(settingsColumnOne(), columnParams(0));
        columns.addView(settingsColumnTwo(), columnParams(20));
        columns.addView(settingsColumnThree(), columnParams(20));
        screen.addView(columns, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View settingsColumnOne() {
        LinearLayout column = Ui.column(this);

        LinearLayout controlsGroup = Ui.group(this, getString(R.string.settings_group_controls));
        LinearLayout controlsList = Ui.list(this);
        View touch = Ui.switchRow(this, getString(R.string.home_toggle_touch), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.TOUCH)), true, Ui.ROW_DP,
                (b, on) -> setControl(ControlsFile.TOUCH, on));
        controlsList.addView(touch);
        defaultFocus = touch;
        controlsList.addView(Ui.switchRow(this, getString(R.string.home_toggle_controller), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.CONTROLLER)), true, Ui.ROW_DP,
                (b, on) -> setControl(ControlsFile.CONTROLLER, on)));
        controlsList.addView(Ui.chevronRow(this, getString(R.string.settings_controls_buttons),
                getString(R.string.settings_controls_buttons_note), Ui.ROW_DP,
                v -> message(getString(R.string.settings_controls_buttons),
                        getString(R.string.settings_controls_legend) + "\n\n"
                                + getString(R.string.settings_controls_rebind))));
        controlsGroup.addView(controlsList);
        column.addView(controlsGroup, Ui.wide(this, 12));

        LinearLayout device = Ui.group(this, getString(R.string.settings_group_device));
        LinearLayout deviceList = Ui.list(this);
        deviceList.addView(Ui.switchRow(this, getString(R.string.settings_keep_awake),
                getString(R.string.settings_keep_awake_note), prefs.getBoolean(PREF_KEEP_AWAKE, true),
                true, Ui.ROW_DP, (b, on) -> prefs.edit().putBoolean(PREF_KEEP_AWAKE, on).apply()));
        device.addView(deviceList);
        column.addView(device, Ui.wide(this, 12));

        LinearLayout launcher = Ui.group(this, getString(R.string.settings_group_launcher));
        LinearLayout launcherList = Ui.list(this);
        launcherList.addView(Ui.valueRow(this, getString(R.string.settings_language), null,
                getString(R.string.settings_language_value), false, Ui.ROW_DENSE_DP,
                v -> message(getString(R.string.settings_language),
                        getString(R.string.settings_language_note))));
        launcherList.addView(Ui.chevronRow(this, getString(R.string.settings_rerun_setup), null,
                Ui.ROW_DENSE_DP, v -> {
                    prefs.edit().putBoolean(PREF_SETUP_DONE, false).apply();
                    go(Section.SETUP);
                }));
        // The second, readable way in. It stays a row of Settings even before it is unlocked.
        launcherList.addView(Ui.chevronRow(this, getString(R.string.settings_debug_row),
                getString(R.string.settings_debug_note), Ui.ROW_DENSE_DP, v -> {
                    unlockDebug();
                    go(Section.DEBUG);
                }));
        launcher.addView(launcherList);
        column.addView(launcher, Ui.wide(this, 0));
        return column;
    }

    private View settingsColumnTwo() {
        LinearLayout column = Ui.column(this);
        column.addView(driverGroup(), Ui.wide(this, 16));

        LinearLayout problems = Ui.group(this, getString(R.string.settings_group_problems));
        LinearLayout list = Ui.list(this);
        list.addView(Ui.chevronRow(this, getString(R.string.report_row),
                getString(R.string.report_row_note), Ui.ROW_DP, v -> reportAProblem()));
        list.addView(Ui.valueRow(this, getString(R.string.strip_last_run), null, lastRunText(),
                false, Ui.ROW_DENSE_DP, v -> {
                    unlockDebug();
                    go(Section.DEBUG);
                }));
        problems.addView(list);
        column.addView(problems, Ui.wide(this, 0));
        return column;
    }

    private View settingsColumnThree() {
        LinearLayout column = Ui.column(this);

        LinearLayout data = Ui.group(this, getString(R.string.settings_group_data));
        LinearLayout dataList = Ui.list(this);
        dataList.addView(Ui.valueRow(this, getString(R.string.settings_profile_row), null,
                profiles.active().name, false, Ui.ROW_DP, v -> go(Section.DATA)));
        dataList.addView(Ui.pathRow(this, getString(R.string.settings_data_folder_row),
                profiles.active().data, Ui.ROW_DP, v -> pickFolder(REQUEST_TREE)));
        data.addView(dataList);
        column.addView(data, Ui.wide(this, 16));

        LinearLayout saves = Ui.group(this, getString(R.string.settings_group_saves));
        saves.addView(savesRows());
        saves.addView(Ui.note(this, getString(R.string.saves_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        column.addView(saves, Ui.wide(this, 0));
        return column;
    }

    /** Back up and restore, the same two rows wherever they are shown. */
    private View savesRows() {
        Profiles.Profile active = profiles.active();
        int saves = RunState.countSaves(active.savesDir(getFilesDir()));
        LinearLayout list = Ui.list(this);
        list.addView(Ui.chevronRow(this, getString(R.string.saves_backup),
                getResources().getQuantityString(R.plurals.saves_backup_note, saves, saves, active.name),
                Ui.ROW_DP, v -> backUpSaves()));
        list.addView(Ui.chevronRow(this, getString(R.string.saves_restore),
                getString(R.string.saves_restore_note), Ui.ROW_DP, v -> restoreSaves()));
        return list;
    }

    /**
     * Settings, then Graphics driver: an exclusive choice of Automatic, the system driver and
     * every driver the player installed, then the one action. Everything it knows comes from
     * {@link GpuDrivers}, which keeps the drivers in the app's private storage; the game opens
     * the chosen one before its renderer starts (engine/nvf-android/src/gpu_driver.rs).
     */
    private View driverGroup() {
        LinearLayout group = Ui.group(this, getString(R.string.settings_driver_row));
        List<String[]> drivers = GpuDrivers.installed(this);
        String mode = GpuDrivers.mode(this);
        LinearLayout list = Ui.list(this);
        list.addView(Ui.radioRow(this, getString(R.string.settings_driver_automatic),
                getString(R.string.settings_driver_automatic_note),
                GpuDrivers.AUTOMATIC.equals(mode), Ui.ROW_DP, v -> {
                    GpuDrivers.select(this, GpuDrivers.AUTOMATIC);
                    show();
                }));
        list.addView(Ui.radioRow(this, getString(R.string.settings_driver_system),
                getString(R.string.settings_driver_system_note), mode == null, Ui.ROW_DP, v -> {
                    GpuDrivers.select(this, null);
                    show();
                }));
        for (String[] driver : drivers) {
            final String id = driver[0];
            LinearLayout row = Ui.radioRow(this, driver[1],
                    getString(R.string.settings_driver_installed_note), id.equals(mode), Ui.ROW_DP,
                    v -> {
                        GpuDrivers.select(this, id);
                        show();
                    });
            row.setOnLongClickListener(v -> {
                confirm(getString(R.string.settings_driver_delete_confirm, driver[1]), () -> {
                    try {
                        GpuDrivers.delete(this, id);
                    } catch (RuntimeException e) {
                        toast(String.valueOf(e.getMessage()));
                    }
                    show();
                });
                return true;
            });
            list.addView(row);
        }
        list.addView(Ui.row(this, getString(R.string.settings_driver_install), null, Ui.indent(this),
                Ui.label(this, ">", Ui.BODY_SP, Ui.INK_3, false), Ui.ROW_DENSE_DP, true,
                v -> pickDriverZip()));
        group.addView(list);
        group.addView(Ui.note(this, getString(drivers.isEmpty() ? R.string.settings_driver_none_note
                : R.string.settings_driver_hold_to_remove)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        return group;
    }

    private void pickDriverZip() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType("application/zip");
        try {
            startActivityForResult(intent, REQUEST_DRIVER_ZIP);
        } catch (RuntimeException e) {
            toast(getString(R.string.error_no_file_app));
        }
    }

    // ---- backing saves up, and reading them back -----------------------------------------

    private File downloadsDir() {
        return Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS);
    }

    /**
     * Copies the active profile's saves into
     * {@code Download/NVForever-backups/<profile>-<date>/}. A copy, never a move: the profile's
     * own folder is left exactly as it was, and nothing anywhere is deleted.
     */
    private void backUpSaves() {
        if (!hasFilesAccess()) {
            toast(getString(R.string.error_need_access));
            return;
        }
        Profiles.Profile active = profiles.active();
        final File saves = active.savesDir(getFilesDir());
        if (RunState.countSaves(saves) == 0) {
            toast(getString(R.string.saves_backup_empty));
            return;
        }
        final File into = SaveBackup.folderFor(downloadsDir(), active.name, System.currentTimeMillis());
        new Thread(() -> {
            String message;
            try {
                int n = SaveBackup.backUp(saves, into);
                message = getResources().getQuantityString(R.plurals.saves_backup_done, n, n,
                        SaveBackup.BACKUPS_TAIL + "/" + into.getName());
            } catch (IOException | SecurityException e) {
                message = getString(R.string.saves_backup_failed, String.valueOf(e.getMessage()));
            }
            final String said = message;
            runOnUiThread(() -> toast(said));
        }, "nvf-save-backup").start();
    }

    /**
     * Reads a backup folder in Download back into the active profile's saves. A save already
     * there is copied to a free {@code .bak} name before it is written over, and the player is
     * told how many that is before anything is written.
     */
    private void restoreSaves() {
        if (!hasFilesAccess()) {
            toast(getString(R.string.error_need_access));
            return;
        }
        List<File> backups = SaveBackup.backups(downloadsDir());
        if (backups.isEmpty()) {
            message(getString(R.string.saves_restore), getString(R.string.saves_restore_none,
                    SaveBackup.BACKUPS_TAIL));
            return;
        }
        String[] names = new String[backups.size()];
        for (int i = 0; i < backups.size(); i++) {
            File backup = backups.get(i);
            int n = SaveBackup.countSaves(backup);
            names[i] = backup.getName() + "   "
                    + getResources().getQuantityString(R.plurals.count_saves_short, n, n);
        }
        new AlertDialog.Builder(this)
                .setTitle(R.string.saves_restore_pick)
                .setItems(names, (d, which) -> confirmRestore(backups.get(which)))
                .setNegativeButton(R.string.common_cancel, null)
                .show();
    }

    private void confirmRestore(final File backup) {
        final File saves = profiles.active().savesDir(getFilesDir());
        int clashes = SaveBackup.clashes(backup, saves);
        Runnable restore = () -> new Thread(() -> {
            String message;
            try {
                int n = SaveBackup.restore(backup, saves);
                message = getResources().getQuantityString(R.plurals.saves_restore_done, n, n);
            } catch (IOException | SecurityException e) {
                message = getString(R.string.saves_restore_failed, String.valueOf(e.getMessage()));
            }
            final String said = message;
            runOnUiThread(() -> {
                toast(said);
                reload();
                show();
            });
        }, "nvf-save-restore").start();
        if (clashes == 0) {
            restore.run();
            return;
        }
        confirm(getResources().getQuantityString(R.plurals.saves_restore_confirm, clashes, clashes),
                restore);
    }

    // ---- Advanced (the hidden Debug screen) ----------------------------------------------

    private void unlockDebug() {
        if (!prefs.getBoolean(PREF_DEBUG_UNLOCKED, false)) {
            prefs.edit().putBoolean(PREF_DEBUG_UNLOCKED, true).apply();
        }
    }

    private View debugScreen() {
        if (!prefs.getBoolean(PREF_DEBUG_CONFIRMED, false)) {
            new AlertDialog.Builder(this)
                    .setMessage(R.string.debug_first_confirm)
                    .setPositiveButton(R.string.debug_open,
                            (d, w) -> prefs.edit().putBoolean(PREF_DEBUG_CONFIRMED, true).apply())
                    .setNegativeButton(R.string.common_cancel, (d, w) -> go(Section.SETTINGS))
                    .show();
        }
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.debug_title), getString(R.string.debug_saves_untouched),
                getString(R.string.legend_back_settings)), Ui.wide(this, 8));
        int on = ControlsFile.testsOn(controls).size();
        if (on > 0) {
            screen.addView(Ui.band(this, getResources().getQuantityString(R.plurals.debug_warn_strip, on, on),
                    Ui.WARN), Ui.wide(this, 10));
        }

        LinearLayout columns = Ui.row(this);
        columns.setGravity(Gravity.TOP);

        LinearLayout one = Ui.column(this);
        LinearLayout engine = Ui.group(this, getString(R.string.debug_group_engine));
        LinearLayout engineList = Ui.list(this);
        View first = testRow(engineList, ControlsFile.SKIP_PIPBOY_WARMUP, R.string.debug_skip_pipboy, true);
        defaultFocus = first;
        testRow(engineList, ControlsFile.SKIP_VATS_OUTLINE, R.string.debug_skip_vats, true);
        engine.addView(engineList);
        one.addView(engine, Ui.wide(this, 12));
        LinearLayout drawing = Ui.group(this, getString(R.string.debug_group_drawing));
        LinearLayout drawingList = Ui.list(this);
        testRow(drawingList, ControlsFile.NO_SOFT_PARTICLES, R.string.debug_no_soft_particles, true);
        testRow(drawingList, ControlsFile.NO_PARTICLES, R.string.debug_no_particles, true);
        testRow(drawingList, ControlsFile.NO_DECALS, R.string.debug_no_decals, true);
        testRow(drawingList, ControlsFile.NO_HEAT_HAZE, R.string.debug_no_heat_haze, true);
        drawing.addView(drawingList);
        drawing.addView(Ui.note(this, getString(R.string.debug_group_drawing_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        one.addView(drawing, Ui.wide(this, 0));
        columns.addView(one, columnParams(0));

        LinearLayout two = Ui.column(this);
        LinearLayout vulkan = Ui.group(this, getString(R.string.debug_group_vulkan));
        LinearLayout vulkanList = Ui.list(this);
        testRow(vulkanList, ControlsFile.VULKAN_VALIDATION, R.string.debug_validation, true);
        // The two deeper checks are children of the layer: focusable, and inert while it is
        // off, which is the dependency the engine itself enforces.
        boolean layer = Boolean.TRUE.equals(controls.get(ControlsFile.VULKAN_VALIDATION));
        childRow(vulkanList, ControlsFile.VULKAN_VALIDATION_SYNC, R.string.debug_validation_sync, layer);
        childRow(vulkanList, ControlsFile.VULKAN_VALIDATION_GPUAV, R.string.debug_validation_gpuav, layer);
        testRow(vulkanList, ControlsFile.GPU_BREADCRUMBS, R.string.debug_breadcrumbs, true);
        // The chunks exist to be marked, so they are a child of the breadcrumbs exactly as the
        // two deeper checks are children of the layer; the engine enforces the same dependency.
        childRow(vulkanList, ControlsFile.GPU_BREADCRUMB_CHUNKS, R.string.debug_breadcrumb_chunks,
                Boolean.TRUE.equals(controls.get(ControlsFile.GPU_BREADCRUMBS)));
        vulkan.addView(vulkanList);
        vulkan.addView(Ui.note(this, getString(R.string.debug_group_vulkan_note)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        two.addView(vulkan, Ui.wide(this, 12));
        LinearLayout diagnostics = Ui.group(this, getString(R.string.debug_group_diagnostics));
        LinearLayout diagnosticsList = Ui.list(this);
        testRow(diagnosticsList, ControlsFile.DIAGNOSTICS, R.string.debug_diagnostics, true);
        diagnostics.addView(diagnosticsList);
        two.addView(diagnostics, Ui.wide(this, 0));
        columns.addView(two, columnParams(20));

        LinearLayout three = Ui.column(this);
        LinearLayout reports = Ui.group(this, getString(R.string.debug_group_reports));
        LinearLayout reportList = Ui.list(this);
        reportList.addView(Ui.chevronRow(this, getString(R.string.report_row), null,
                Ui.ROW_DENSE_DP, v -> reportAProblem()));
        reportList.addView(Ui.chevronRow(this, getString(R.string.debug_export_log), null,
                Ui.ROW_DENSE_DP, v -> chooseDebugLogDestination()));
        reportList.addView(Ui.chevronRow(this, getString(R.string.debug_copy_summary), null,
                Ui.ROW_DENSE_DP, v -> copyDebugSummary()));
        reports.addView(reportList);
        reports.addView(Ui.note(this, getString(R.string.debug_privacy)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        three.addView(reports, Ui.wide(this, 16));
        LinearLayout reset = Ui.group(this, getString(R.string.debug_group_reset));
        LinearLayout resetList = Ui.list(this);
        resetList.addView(Ui.chevronRow(this, getString(R.string.debug_tests_off), null,
                Ui.ROW_DENSE_DP, v -> {
                    ControlsFile.clearTests(controls);
                    saveControls();
                    toast(getString(R.string.debug_tests_off_done));
                    show();
                }));
        reset.addView(resetList);
        three.addView(reset, Ui.wide(this, 0));
        columns.addView(three, columnParams(20));

        screen.addView(columns, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    /**
     * One crash-test switch. The long explanation each of these has is read on a press and
     * hold rather than under the label, which is what lets the screen hold them all at once.
     */
    private View testRow(LinearLayout into, String key, int title, boolean enabled) {
        return testRow(into, key, title, enabled, false);
    }

    /** A switch that depends on the one above it: indented, and inert while that one is off. */
    private View childRow(LinearLayout into, String key, int title, boolean enabled) {
        return testRow(into, key, title, enabled, true);
    }

    private View testRow(LinearLayout into, String key, int title, boolean enabled, boolean child) {
        LinearLayout row = Ui.switchRow(this, (child ? "   " : "") + getString(title), null,
                Boolean.TRUE.equals(controls.get(key)), enabled, Ui.ROW_DENSE_DP, (b, on) -> {
                    setControl(key, on);
                    show();
                });
        row.setOnLongClickListener(v -> {
            message(getString(title), getString(noteFor(key)));
            return true;
        });
        into.addView(row);
        return row;
    }

    /** The long explanation of a crash test, shown on a press and hold. */
    private static int noteFor(String key) {
        switch (key) {
            case ControlsFile.SKIP_VATS_OUTLINE: return R.string.debug_skip_vats_note;
            case ControlsFile.VULKAN_VALIDATION: return R.string.debug_validation_note;
            case ControlsFile.VULKAN_VALIDATION_SYNC: return R.string.debug_validation_sync_note;
            case ControlsFile.VULKAN_VALIDATION_GPUAV: return R.string.debug_validation_gpuav_note;
            case ControlsFile.GPU_BREADCRUMBS: return R.string.debug_breadcrumbs_note;
            case ControlsFile.GPU_BREADCRUMB_CHUNKS: return R.string.debug_breadcrumb_chunks_note;
            case ControlsFile.NO_SOFT_PARTICLES: return R.string.debug_no_soft_particles_note;
            case ControlsFile.NO_PARTICLES: return R.string.debug_no_particles_note;
            case ControlsFile.NO_DECALS: return R.string.debug_no_decals_note;
            case ControlsFile.NO_HEAT_HAZE: return R.string.debug_no_heat_haze_note;
            case ControlsFile.DIAGNOSTICS: return R.string.debug_diagnostics_note;
            default: return R.string.debug_skip_pipboy_note;
        }
    }

    // ---- About -------------------------------------------------------------------------

    private View aboutScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(header(getString(R.string.about_title), getString(R.string.about_detail_meta),
                getString(R.string.legend_back_menu)), Ui.wide(this, 10));

        LinearLayout columns = Ui.row(this);
        columns.setGravity(Gravity.TOP);

        LinearLayout words = Ui.column(this);
        words.addView(Ui.body(this, getString(R.string.about_disclaimer)));
        words.addView(Ui.note(this, getString(R.string.about_trademark)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        words.addView(Ui.note(this, getString(R.string.about_extender)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        words.addView(Ui.micro(this, getString(R.string.about_build) + "  " + buildChipText(), Ui.INK_3),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT, 0, 10, 0));
        columns.addView(words, columnParams(0));

        LinearLayout licences = Ui.group(this, getString(R.string.about_licences));
        LinearLayout list = Ui.list(this);
        View first = licenceRow(list, R.string.about_licence_nvforever, "gpl-3.0.txt");
        defaultFocus = first;
        licenceRow(list, R.string.about_licence_validation, "vulkan-validation-apache-2.0.txt");
        licenceRow(list, R.string.about_licence_sans, "inter-ofl-1.1.txt");
        licenceRow(list, R.string.about_licence_font, "jetbrains-mono-ofl-1.1.txt");
        licenceRow(list, R.string.about_licence_crates, "rust-crates.txt");
        licences.addView(list);
        columns.addView(licences, columnParams(20));

        screen.addView(columns, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View licenceRow(LinearLayout into, int title, String asset) {
        View row = Ui.chevronRow(this, getString(title), getString(R.string.about_licence_read),
                Ui.ROW_DP, v -> showLicence(getString(title), asset));
        into.addView(row);
        return row;
    }

    /** The full text, from a bundled asset. Nothing here reaches the network. */
    private void showLicence(String title, String asset) {
        String text;
        try (java.io.InputStream in = getAssets().open("licences/" + asset)) {
            byte[] bytes = new byte[in.available()];
            int read = in.read(bytes);
            text = new String(bytes, 0, Math.max(read, 0), StandardCharsets.UTF_8);
        } catch (IOException e) {
            text = String.valueOf(e.getMessage());
        }
        TextView view = Ui.micro(this, text, Ui.INK);
        int pad = Ui.dp(this, 12);
        view.setPaddingRelative(pad, pad, pad, pad);
        new AlertDialog.Builder(this)
                .setTitle(title)
                .setView(Ui.page(this, view))
                .setPositiveButton(R.string.common_close, null)
                .show();
    }

    /** One paragraph the player asked to read, and one way out of it. */
    private void message(String title, String text) {
        new AlertDialog.Builder(this)
                .setTitle(title)
                .setMessage(text)
                .setPositiveButton(R.string.common_close, null)
                .show();
    }

    // ---- the controls file -------------------------------------------------------------

    private File controlsFile() {
        return new File(Environment.getExternalStorageDirectory(), "NVForever/nvforever-controls.txt");
    }

    private void setControl(String key, boolean on) {
        controls.put(key, on);
        saveControls();
    }

    private void saveControls() {
        if (!hasFilesAccess()) {
            toast(getString(R.string.error_need_access));
            controls = ControlsFile.read(controlsFile());
            return;
        }
        try {
            ControlsFile.write(controlsFile(), controls);
        } catch (IOException e) {
            toast(getString(R.string.error_save_controls, e.toString()));
            controls = ControlsFile.read(controlsFile());
        }
    }

    // ---- permission --------------------------------------------------------------------

    private boolean hasFilesAccess() {
        if (Build.VERSION.SDK_INT >= 30) {
            return Environment.isExternalStorageManager();
        }
        return checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
    }

    private void requestFilesAccess() {
        if (Build.VERSION.SDK_INT >= 30) {
            try {
                startActivity(new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                        Uri.parse("package:" + getPackageName())));
            } catch (Exception e) {
                startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
            }
        } else {
            requestPermissions(new String[] {
                Manifest.permission.READ_EXTERNAL_STORAGE,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            }, REQUEST_LEGACY_PERMISSION);
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        reload();
        show();
    }

    // ---- folders -----------------------------------------------------------------------

    private File defaultDataDir() {
        return new File(Environment.getExternalStorageDirectory(), Profiles.DEFAULT_DATA_TAIL);
    }

    private void pickFolder(int request) {
        try {
            startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), request);
        } catch (RuntimeException e) {
            toast(getString(R.string.error_no_file_app));
        }
    }

    private void createDefaultFolder() {
        if (!hasFilesAccess()) {
            toast(getString(R.string.error_need_access));
            return;
        }
        File dir = defaultDataDir();
        dir.mkdirs();
        profiles.active().data = dir.getPath();
        saveProfiles();
        reload();
        show();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        boolean ok = resultCode == RESULT_OK && data != null && data.getData() != null;
        if (requestCode == REQUEST_EXPORT_LOG) {
            if (ok) {
                exportDebugLog(data.getData());
            }
            return;
        }
        if (requestCode == REQUEST_DRIVER_ZIP) {
            if (ok) {
                try {
                    GpuDrivers.install(this, data.getData());
                } catch (IOException | RuntimeException e) {
                    toast(String.valueOf(e.getMessage()));
                }
            }
            show();
            return;
        }
        if (ok && (requestCode == REQUEST_TREE || requestCode == REQUEST_NEW_PROFILE_TREE)) {
            File picked = treeToPath(data.getData());
            if (picked != null) {
                File folder = DataCheck.normalize(picked);
                if (requestCode == REQUEST_NEW_PROFILE_TREE && pendingProfileName != null) {
                    Profiles.Profile from = profiles.active();
                    Profiles.Profile added = profiles.add(pendingProfileName, folder);
                    pendingProfileName = null;
                    profiles.setActive(added.id);
                    offerCopySaves(from, added);
                } else {
                    profiles.active().data = folder.getPath();
                }
                saveProfiles();
            }
        }
        reload();
        show();
    }

    /**
     * A new profile starts with no save. Offer the saves of the profile the player was on, as
     * a copy: the folder copied from is left exactly as it was, and nothing is ever moved.
     */
    private void offerCopySaves(final Profiles.Profile source, final Profiles.Profile added) {
        if (source.id.equals(added.id) || RunState.countSaves(source.savesDir(getFilesDir())) == 0) {
            return;
        }
        new AlertDialog.Builder(this)
                .setMessage(R.string.data_profiles_copy_saves)
                .setPositiveButton(R.string.common_yes, (d, w) -> {
                    try {
                        int n = Profiles.copySaves(source.savesDir(getFilesDir()), added.savesDir(getFilesDir()));
                        toast(getResources().getQuantityString(R.plurals.data_profiles_copied, n, n));
                    } catch (IOException e) {
                        toast(getString(R.string.error_copy_saves, e.toString()));
                    }
                })
                .setNegativeButton(R.string.common_cancel, null)
                .show();
    }

    /** content://.../tree/primary%3AGames%2FFNV -> /storage/emulated/0/Games/FNV. */
    private static File treeToPath(Uri uri) {
        String id = DocumentsContract.getTreeDocumentId(uri);
        int colon = id.indexOf(':');
        String volume = colon < 0 ? id : id.substring(0, colon);
        String rel = colon < 0 ? "" : id.substring(colon + 1);
        File base = "primary".equals(volume) ? Environment.getExternalStorageDirectory() : new File("/storage/" + volume);
        return rel.isEmpty() ? base : new File(base, rel);
    }

    // ---- starting the game -------------------------------------------------------------

    /** PLAY, or one chosen save: one start's save, through files/android-continue.txt. */
    private void startGame(File save) {
        if (!hasFilesAccess()) {
            toast(getString(R.string.error_need_access));
            return;
        }
        // The shared folder holds the option files; make sure it exists.
        new File(Environment.getExternalStorageDirectory(), "NVForever").mkdirs();
        // The two pointers the engine reads: the active profile's Data folder and its home.
        saveProfiles();
        File request = new File(getFilesDir(), "android-continue.txt");
        try {
            if (save != null) {
                Profiles.writeWhole(request, save.getPath() + "\n");
            } else {
                request.delete();
            }
        } catch (IOException e) {
            toast(getString(R.string.error_resume));
            return;
        }
        startActivity(new Intent(this, GameActivity.class)
                .putExtra(GameActivity.EXTRA_KEEP_AWAKE, prefs.getBoolean(PREF_KEEP_AWAKE, true))
                .addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT | Intent.FLAG_ACTIVITY_SINGLE_TOP));
    }

    /**
     * The game asked for a fresh process to take over a load (handoff.rs). The request is in
     * the active profile's home, and only this app's own game process is ever ended.
     */
    private void resumeRequested(Intent intent) {
        if (!intent.getBooleanExtra("resumeGame", false)) {
            return;
        }
        intent.removeExtra("resumeGame");
        File request = new File(profiles.active().homeDir(getFilesDir()), "android-restart.json");
        if (!request.isFile()) {
            return;
        }
        try {
            byte[] bytes = java.nio.file.Files.readAllBytes(request.toPath());
            org.json.JSONObject packet = new org.json.JSONObject(new String(bytes, StandardCharsets.UTF_8));
            int pid = packet.getInt("pid");
            org.json.JSONArray args = packet.getJSONArray("args");
            if (pid <= 0 || pid == android.os.Process.myPid() || args.length() == 0) {
                throw new IOException("Invalid restart request");
            }
            File next = new File(getFilesDir(), "android-next-run.json");
            File temp = new File(getFilesDir(), "android-next-run.tmp");
            java.nio.file.Files.write(temp.toPath(), args.toString().getBytes(StandardCharsets.UTF_8));
            java.nio.file.Files.move(temp.toPath(), next.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            if (!request.delete()) {
                throw new IOException("Cannot consume restart request");
            }
            // Only end our own game process, identified by both UID and process name.
            android.app.ActivityManager manager = (android.app.ActivityManager) getSystemService(ACTIVITY_SERVICE);
            List<android.app.ActivityManager.RunningAppProcessInfo> processes = manager.getRunningAppProcesses();
            if (processes != null) {
                for (android.app.ActivityManager.RunningAppProcessInfo process : processes) {
                    if (process.pid == pid && process.uid == android.os.Process.myUid()
                            && process.processName.equals(getPackageName() + ":game")) {
                        android.os.Process.killProcess(pid);
                    }
                }
            }
            final long deadline = android.os.SystemClock.uptimeMillis() + 5000;
            final android.os.Handler handler = new android.os.Handler(android.os.Looper.getMainLooper());
            handler.postDelayed(new Runnable() {
                @Override public void run() {
                    boolean alive = false;
                    List<android.app.ActivityManager.RunningAppProcessInfo> running = manager.getRunningAppProcesses();
                    if (running != null) {
                        for (android.app.ActivityManager.RunningAppProcessInfo process : running) {
                            if (process.pid == pid && process.uid == android.os.Process.myUid()
                                    && process.processName.equals(getPackageName() + ":game")) {
                                alive = true;
                            }
                        }
                    }
                    if (!alive) {
                        startActivity(new Intent(LauncherActivity.this, GameActivity.class)
                                .putExtra(GameActivity.EXTRA_KEEP_AWAKE, prefs.getBoolean(PREF_KEEP_AWAKE, true))
                                .addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT | Intent.FLAG_ACTIVITY_SINGLE_TOP));
                    } else if (android.os.SystemClock.uptimeMillis() < deadline) {
                        handler.postDelayed(this, 100);
                    } else {
                        next.delete();
                        toast(getString(R.string.error_restart));
                    }
                }
            }, 250);
        } catch (Exception e) {
            android.util.Log.e("NVForever", "Restarting game", e);
            toast(getString(R.string.error_resume));
        }
    }

    // ---- reports -----------------------------------------------------------------------

    private File logsDir() {
        return profiles.active().logsDir(getFilesDir());
    }

    /**
     * The friendly front door to the debug log: say what a report is and where it goes, then
     * write the log where the player chooses. The two raw actions stay on Advanced.
     */
    private void reportAProblem() {
        new AlertDialog.Builder(this)
                .setTitle(R.string.report_row)
                .setMessage(getString(R.string.report_where) + "\n\n" + getString(R.string.debug_privacy))
                .setPositiveButton(R.string.report_save_log, (d, w) -> chooseDebugLogDestination())
                .setNegativeButton(R.string.common_cancel, null)
                .show();
    }

    private void chooseDebugLogDestination() {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_TITLE, "NVForever-debug-" + BuildConfig.VERSION_NAME + ".txt");
        try {
            startActivityForResult(intent, REQUEST_EXPORT_LOG);
        } catch (RuntimeException e) {
            toast(getString(R.string.error_no_file_app));
        }
    }

    private void exportDebugLog(Uri destination) {
        String header = getString(R.string.app_name) + " " + BuildConfig.VERSION_NAME
                + "\nAndroid " + Build.VERSION.RELEASE + " (API " + Build.VERSION.SDK_INT + ")"
                + " / " + Build.MANUFACTURER + " " + Build.MODEL
                + "\nProfile: " + profiles.active().name
                // Which Vulkan driver the next start uses, and why the last one fell back if it
                // did: the engine's own "gpu driver:" line in the run log below says what the
                // adapter then turned out to be (docs/odin-debug.md).
                + "\n" + GpuDriversDebug.summary(this);
        final File logs = logsDir();
        new Thread(() -> {
            int result;
            try (java.io.OutputStream stream = getContentResolver().openOutputStream(destination, "wt")) {
                if (stream == null) {
                    throw new IOException("No writable destination");
                }
                try (java.io.Writer out = new java.io.OutputStreamWriter(stream, StandardCharsets.UTF_8)) {
                    DebugReport.write(logs, header, out);
                }
                result = R.string.error_export_log_done;
            } catch (IOException | SecurityException e) {
                result = R.string.error_export_log;
            }
            final int message = result;
            runOnUiThread(() -> toast(getString(message)));
        }, "nvf-log-export").start();
    }

    private void copyDebugSummary() {
        StringBuilder summary = new StringBuilder(getString(R.string.app_name) + " " + BuildConfig.VERSION_NAME
                + "\nAndroid " + Build.VERSION.RELEASE + " / " + Build.MODEL
                + "\nProfile: " + profiles.active().name
                + "\n" + GpuDriversDebug.summary(this));
        File[] logs = logsDir().listFiles(
                (directory, name) -> name.startsWith("nvforever-") && name.endsWith(".log"));
        File newest = null;
        if (logs != null) {
            for (File log : logs) {
                if (log.isFile() && (newest == null || log.lastModified() > newest.lastModified())) {
                    newest = log;
                }
            }
        }
        if (newest != null) {
            try (java.io.RandomAccessFile input = new java.io.RandomAccessFile(newest, "r")) {
                int count = (int) Math.min(input.length(), 8192L);
                byte[] tail = new byte[count];
                input.seek(input.length() - count);
                input.readFully(tail);
                summary.append("\n\n").append(new String(tail, StandardCharsets.UTF_8));
            } catch (IOException ignored) {
                summary.append("\n(the latest run log could not be read)");
            }
        } else {
            summary.append("\n(no engine run log yet)");
        }
        android.content.ClipboardManager clipboard = getSystemService(android.content.ClipboardManager.class);
        clipboard.setPrimaryClip(android.content.ClipData.newPlainText("NVForever debug summary", summary.toString()));
        toast(getString(R.string.error_copy_done));
    }

    private void toast(String message) {
        Toast.makeText(this, message, Toast.LENGTH_LONG).show();
    }
}
