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
 * The screen the app opens on: the launcher.
 *
 * <p>Six destinations, never more than two levels deep. Setup on a first run (all-files
 * access, the Data folder, the file check), then Home (CONTINUE, PLAY and a status card that
 * answers "will it work"), the Data area (data profiles), Settings (launcher options only),
 * About (the disclaimer and the licences), and a hidden Debug screen holding the crash-test
 * switches that used to sit on the first screen.
 *
 * <p>What it writes, and nothing else: {@code files/profiles.txt}, {@code files/data-path.txt}
 * and {@code files/profile.txt} (the engine reads the last two, nvf-android/src/paths.rs),
 * {@code files/android-continue.txt} for one CONTINUE, and the shared
 * {@code NVForever/nvforever-controls.txt}. It never writes into a Data folder, never moves
 * a save, and never touches a plugin list.
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

    /** The launcher's screens. Setup and Debug are not in the shoulder-button round. */
    private enum Section { SETUP, HOME, DATA, SETTINGS, ABOUT, DEBUG }

    private static final Section[] PAGED = {Section.HOME, Section.DATA, Section.SETTINGS, Section.ABOUT};

    private SharedPreferences prefs;
    private Profiles profiles;
    private Map<String, Boolean> controls = new LinkedHashMap<>();
    private DataCheck.Result check;
    private Section section = Section.HOME;
    /** The name of the profile being created, between naming it and picking its folder. */
    private String pendingProfileName;

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
        root.setBackgroundColor(Ui.BG);
        int pad = Ui.dp(this, 16);
        root.setPaddingRelative(pad, pad, pad, pad);
        root.addView(topBar());

        LinearLayout content = Ui.column(this);
        content.addView(buildSection(), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 12, 12));
        root.addView(content, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        root.addView(bottomBar());
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

    private View topBar() {
        LinearLayout bar = Ui.row(this);
        LinearLayout names = Ui.column(this);
        TextView wordmark = Ui.label(this, getString(R.string.app_name), Ui.TITLE_SP, Ui.AMBER, true);
        wordmark.setLetterSpacing(0.22f);
        names.addView(wordmark);
        TextView tagline = Ui.note(this, getString(R.string.app_tagline));
        tagline.setMaxLines(2);
        names.addView(tagline);
        bar.addView(names, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        LinearLayout chips = Ui.column(this);
        chips.setGravity(Gravity.END);
        TextView build = Ui.label(this, buildChipText(), Ui.SMALL_SP, Ui.AMBER_DIM, false);
        build.setPaddingRelative(Ui.dp(this, 8), Ui.dp(this, 6), Ui.dp(this, 8), Ui.dp(this, 6));
        build.setBackground(Ui.focusable(this, Ui.PANEL, Ui.LINE));
        build.setFocusable(true);
        build.setOnClickListener(v -> {});
        build.setOnLongClickListener(v -> {
            unlockDebug();
            return true;
        });
        build.setContentDescription(buildChipText() + ". Hold to show the debug screen.");
        chips.addView(build);
        int testsOn = ControlsFile.testsOn(controls).size();
        if (testsOn > 0) {
            chips.addView(Ui.label(this, getString(R.string.home_tests_on_chip, testsOn), Ui.SMALL_SP, Ui.WARN, true),
                    Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 4, 0));
        }
        bar.addView(chips);
        return bar;
    }

    private String buildChipText() {
        return BuildConfig.VERSION_NAME + " " + Build.SUPPORTED_ABIS[0] + " " + Build.MODEL;
    }

    private View bottomBar() {
        LinearLayout bar = Ui.column(this);
        LinearLayout buttons = Ui.row(this);
        for (Section s : PAGED) {
            if (s == section) {
                continue;
            }
            TextView b = Ui.button(this, sectionName(s), v -> go(s));
            buttons.addView(b, barItem());
        }
        if (prefs.getBoolean(PREF_DEBUG_UNLOCKED, false) && section != Section.DEBUG) {
            buttons.addView(Ui.button(this, sectionName(Section.DEBUG), v -> go(Section.DEBUG)), barItem());
        }
        bar.addView(buttons);
        bar.addView(Ui.note(this, getString(section == Section.HOME ? R.string.legend_home : R.string.legend_other)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        return bar;
    }

    private LinearLayout.LayoutParams barItem() {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        p.setMarginEnd(Ui.dp(this, 8));
        return p;
    }

    private String sectionName(Section s) {
        switch (s) {
            case SETUP: return getString(R.string.section_setup);
            case DATA: return getString(R.string.section_data);
            case SETTINGS: return getString(R.string.section_settings);
            case ABOUT: return getString(R.string.section_about);
            case DEBUG: return getString(R.string.section_debug);
            default: return getString(R.string.section_home);
        }
    }

    private void go(Section s) {
        section = s;
        defaultFocus = null;
        show();
    }

    private View buildSection() {
        defaultFocus = null;
        switch (section) {
            case SETUP: return setupScreen();
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

    // ---- Home --------------------------------------------------------------------------

    private View homeScreen() {
        LinearLayout screen = Ui.row(this);
        screen.setGravity(Gravity.TOP);

        LinearLayout left = Ui.column(this);
        Profiles.Profile active = profiles.active();
        LinearLayout chip = Ui.buttonWithNote(this,
                getString(R.string.home_profile_label) + "  " + active.name,
                getString(R.string.home_profile_change), v -> go(Section.DATA));
        left.addView(chip, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        RunState.Save newest = RunState.newestSave(active.savesDir(getFilesDir()));
        if (newest != null) {
            View hero = Ui.hero(this, getString(R.string.home_continue),
                    getString(R.string.home_continue_sub, newest.name(), ago(newest.modified), active.name),
                    v -> startGame(newest.file));
            left.addView(hero, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));
            defaultFocus = hero;
            View play = Ui.buttonWithNote(this, getString(R.string.home_play),
                    getString(R.string.home_play_sub), v -> startGame(null));
            left.addView(play, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));
        } else {
            // No save in this profile: PLAY is the hero, and CONTINUE is absent rather than
            // a giant dead control a D-pad can land on.
            View play = Ui.hero(this, getString(R.string.home_play), getString(R.string.home_play_sub),
                    v -> startGame(null));
            left.addView(play, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));
            defaultFocus = play;
        }

        left.addView(Ui.switchRow(this, getString(R.string.home_toggle_touch), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.TOUCH)), true,
                (b, on) -> setControl(ControlsFile.TOUCH, on)));
        left.addView(Ui.switchRow(this, getString(R.string.home_toggle_controller), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.CONTROLLER)), true,
                (b, on) -> setControl(ControlsFile.CONTROLLER, on)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        left.addView(Ui.note(this, getString(R.string.home_toggles_note)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));

        screen.addView(left, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 0.62f));
        LinearLayout right = statusCard();
        LinearLayout.LayoutParams rp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 0.38f);
        rp.setMarginStart(Ui.dp(this, 12));
        screen.addView(right, rp);
        return screen;
    }

    private LinearLayout statusCard() {
        LinearLayout card = Ui.panel(this, getString(R.string.home_status_title));
        Profiles.Profile active = profiles.active();
        card.addView(Ui.readout(this, getString(R.string.home_status_data), active.data, Ui.TEXT));
        card.addView(Ui.readout(this, "", checkLine(), check.ready() ? Ui.TEXT : Ui.BAD));
        card.addView(Ui.readout(this, getString(R.string.home_status_saves),
                getResources().getQuantityString(R.plurals.count_saves,
                        RunState.countSaves(active.savesDir(getFilesDir())),
                        RunState.countSaves(active.savesDir(getFilesDir()))), Ui.TEXT));
        boolean access = hasFilesAccess();
        card.addView(Ui.readout(this, getString(R.string.home_status_access),
                getString(access ? R.string.home_status_access_granted : R.string.home_status_access_missing),
                access ? Ui.TEXT : Ui.BAD));
        RunState.LastRun last = RunState.lastRun(active.logsDir(getFilesDir()));
        int lastText = last == RunState.LastRun.NONE ? R.string.home_last_run_none
                : last == RunState.LastRun.OK ? R.string.home_last_run_ok : R.string.home_last_run_crash;
        card.addView(Ui.readout(this, getString(R.string.home_status_last_run), getString(lastText),
                last == RunState.LastRun.CRASHED ? Ui.WARN : Ui.TEXT));
        if (last == RunState.LastRun.CRASHED) {
            card.addView(Ui.button(this, getString(R.string.home_last_run_crash_action), v -> {
                unlockDebug();
                go(Section.DEBUG);
            }), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        }
        String fellBack = GpuDrivers.lastStartFellBack(this);
        if (fellBack != null) {
            card.addView(Ui.strip(this, getString(R.string.home_driver_fell_back, fellBack), Ui.WARN),
                    Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        }
        card.addView(Ui.button(this, getString(R.string.home_open_data), v -> go(Section.DATA)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 10, 0));
        card.addView(Ui.button(this, getString(R.string.home_export_log), v -> chooseDebugLogDestination()),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        return card;
    }

    /** The one-line verdict on the active profile's folder, used on Home and in the Data area. */
    private String checkLine() {
        if (check.closed) {
            return getString(R.string.setup_blocked_android_data);
        }
        if (!check.exists) {
            return getString(R.string.check_folder_unreadable);
        }
        String base = Ui.mark(check.baseMasterFound()) + " "
                + getString(check.baseMasterFound() ? R.string.check_base_found : R.string.check_base_missing);
        String archives = Ui.mark(check.archivesFound() == check.archivesTotal()) + " "
                + getString(R.string.check_archives, check.archivesFound(), check.archivesTotal());
        String addons = getString(R.string.check_addons, check.addonsFound(), DataCheck.ADDON_MASTERS.length);
        return base + "\n" + archives + "\n" + addons;
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

    // ---- Setup -------------------------------------------------------------------------

    private View setupScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(Ui.heading(this, getString(R.string.setup_title)));
        screen.addView(Ui.note(this, getString(R.string.setup_intro)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 4, 10));

        boolean access = hasFilesAccess();
        LinearLayout step1 = Ui.panel(this, getString(R.string.setup_step_access_title));
        step1.addView(Ui.note(this, getString(R.string.setup_step_access_why)));
        if (Build.VERSION.SDK_INT < 30) {
            step1.addView(Ui.note(this, getString(R.string.setup_step_access_legacy)));
        }
        if (access) {
            step1.addView(Ui.body(this, Ui.mark(true) + " " + getString(R.string.setup_step_access_granted)));
        } else {
            View grant = Ui.buttonWithNote(this, getString(R.string.setup_step_access_grant),
                    getString(R.string.setup_step_access_return), v -> requestFilesAccess());
            step1.addView(grant, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
            defaultFocus = grant;
        }
        screen.addView(step1, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout step2 = Ui.panel(this, getString(R.string.setup_step_folder_title));
        step2.addView(Ui.note(this, getString(R.string.setup_step_folder_why)));
        step2.addView(Ui.body(this, getString(R.string.setup_step_folder_chosen, profiles.active().data)));
        step2.addView(Ui.note(this, getString(R.string.setup_step_folder_profile_note)));
        View choose = Ui.button(this, getString(R.string.setup_step_folder_choose), v -> pickFolder(REQUEST_TREE));
        choose.setEnabled(access);
        step2.addView(choose, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        View create = Ui.buttonWithNote(this, getString(R.string.setup_step_folder_create),
                getString(R.string.setup_step_folder_create_note), v -> createDefaultFolder());
        create.setEnabled(access);
        step2.addView(create, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        if (access && defaultFocus == null) {
            defaultFocus = choose;
        }
        screen.addView(step2, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout step3 = Ui.panel(this, getString(R.string.setup_step_check_title));
        step3.addView(Ui.note(this, getString(R.string.setup_step_check_why)));
        step3.addView(checkList());
        if (check.ready()) {
            step3.addView(Ui.body(this, Ui.mark(true) + " " + getString(R.string.setup_step_check_ready)));
            View play = Ui.hero(this, getString(R.string.home_play), getString(R.string.home_play_sub), v -> {
                prefs.edit().putBoolean(PREF_SETUP_DONE, true).apply();
                startGame(null);
            });
            step3.addView(play, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
            defaultFocus = play;
        } else {
            step3.addView(Ui.label(this, Ui.mark(false) + " " + getString(R.string.setup_step_check_blocked),
                    Ui.BODY_SP, Ui.BAD, false));
            step3.addView(Ui.button(this, getString(R.string.common_verify), v -> {
                reload();
                show();
            }), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
            // The escape hatch stays, demoted: it is useful, and it is not the happy path.
            TextView anyway = Ui.label(this, getString(R.string.setup_start_anyway), Ui.SMALL_SP, Ui.WEAK, false);
            anyway.setPaddingRelative(Ui.dp(this, 12), Ui.dp(this, 10), Ui.dp(this, 12), Ui.dp(this, 10));
            anyway.setBackground(Ui.focusable(this, Ui.BG, Ui.LINE));
            anyway.setFocusable(true);
            anyway.setEnabled(access && !check.closed);
            anyway.setOnClickListener(v -> startGame(null));
            step3.addView(anyway, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        }
        screen.addView(step3, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    /** The tick list of required files and detected add-ons, with sizes. */
    private View checkList() {
        LinearLayout list = Ui.column(this);
        if (check.closed) {
            list.addView(Ui.label(this, getString(R.string.setup_blocked_android_data), Ui.BODY_SP, Ui.BAD, false));
            return list;
        }
        if (!check.exists) {
            list.addView(Ui.label(this, getString(R.string.setup_blocked_missing_folder), Ui.BODY_SP, Ui.BAD, false));
            return list;
        }
        for (DataCheck.Entry e : check.required) {
            list.addView(Ui.readout(this, Ui.mark(e.present) + " " + e.name,
                    e.present ? size(e.size) : getString(R.string.check_missing_file, e.name),
                    e.present ? Ui.WEAK : Ui.BAD));
        }
        list.addView(Ui.readout(this, getString(R.string.check_addons, check.addonsFound(),
                DataCheck.ADDON_MASTERS.length), addonNames(), Ui.WEAK));
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

    /** A file size, in the units the system formats for this language. */
    private String size(long bytes) {
        return android.text.format.Formatter.formatShortFileSize(this, bytes);
    }

    // ---- Data --------------------------------------------------------------------------

    private View dataScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(Ui.heading(this, getString(R.string.data_title)));
        LinearLayout panel = Ui.panel(this, getString(R.string.data_profiles_title));
        panel.addView(Ui.note(this, getString(R.string.data_profiles_note)));
        for (Profiles.Profile p : profiles.all()) {
            panel.addView(profileRow(p), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        }
        View add = Ui.button(this, getString(R.string.data_profiles_new), v -> newProfile());
        panel.addView(add, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 10, 0));
        screen.addView(panel, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 10));

        // Phase 2 lives here. The row is shown, not hidden, so the shape of the area is honest.
        LinearLayout soon = Ui.panel(this, getString(R.string.data_mods_soon));
        soon.addView(Ui.note(this, getString(R.string.data_mods_soon_note)));
        screen.addView(soon, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View profileRow(Profiles.Profile p) {
        boolean active = p.id.equals(profiles.active().id);
        DataCheck.Result scan = active ? check : DataCheck.scan(p.dataDir());
        String badge = !scan.exists ? getString(R.string.data_profiles_unavailable)
                : Ui.mark(scan.ready()) + " " + getString(R.string.check_archives,
                        scan.archivesFound(), scan.archivesTotal())
                        + ", " + getString(R.string.check_addons, scan.addonsFound(), DataCheck.ADDON_MASTERS.length);
        LinearLayout row = Ui.column(this);
        int pad = Ui.dp(this, 12);
        row.setPaddingRelative(pad, pad / 2, pad, pad / 2);
        row.setBackground(Ui.focusable(this, Ui.PANEL, active ? Ui.AMBER_EDGE : Ui.LINE));
        row.setFocusable(true);
        row.addView(Ui.label(this, p.name + (active ? "   " + getString(R.string.data_profiles_active) : ""),
                Ui.BODY_SP, active ? Ui.AMBER : Ui.TEXT, true));
        row.addView(Ui.note(this, p.data));
        row.addView(Ui.note(this, badge));
        row.addView(Ui.note(this, getResources().getQuantityString(R.plurals.count_saves,
                RunState.countSaves(p.savesDir(getFilesDir())), RunState.countSaves(p.savesDir(getFilesDir())))));
        row.setContentDescription(p.name + ". " + p.data + ". " + badge);
        row.setOnClickListener(v -> profileActions(p, scan));
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
        field.setTextColor(Ui.TEXT);
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
        screen.addView(Ui.heading(this, getString(R.string.settings_title)));
        screen.addView(Ui.note(this, getString(R.string.settings_note)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 4, 10));

        LinearLayout controlsPanel = Ui.panel(this, getString(R.string.settings_group_controls));
        View touch = Ui.switchRow(this, getString(R.string.home_toggle_touch), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.TOUCH)), true,
                (b, on) -> setControl(ControlsFile.TOUCH, on));
        controlsPanel.addView(touch);
        defaultFocus = touch;
        controlsPanel.addView(Ui.switchRow(this, getString(R.string.home_toggle_controller), null,
                Boolean.TRUE.equals(controls.get(ControlsFile.CONTROLLER)), true,
                (b, on) -> setControl(ControlsFile.CONTROLLER, on)),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        controlsPanel.addView(Ui.note(this, getString(R.string.settings_controls_legend)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        controlsPanel.addView(Ui.note(this, getString(R.string.settings_controls_rebind)));
        screen.addView(controlsPanel, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout device = Ui.panel(this, getString(R.string.settings_group_device));
        device.addView(Ui.switchRow(this, getString(R.string.settings_keep_awake),
                getString(R.string.settings_keep_awake_note), prefs.getBoolean(PREF_KEEP_AWAKE, true), true,
                (b, on) -> prefs.edit().putBoolean(PREF_KEEP_AWAKE, on).apply()));
        device.addView(driverRow(), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        screen.addView(device, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout data = Ui.panel(this, getString(R.string.settings_group_data));
        data.addView(Ui.buttonWithNote(this,
                getString(R.string.settings_profile_row) + ": " + profiles.active().name,
                getString(R.string.settings_profile_note), v -> go(Section.DATA)));
        screen.addView(data, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout launcher = Ui.panel(this, getString(R.string.settings_group_launcher));
        launcher.addView(Ui.buttonWithNote(this, getString(R.string.settings_language),
                getString(R.string.settings_language_note), v -> {}));
        launcher.addView(Ui.button(this, getString(R.string.settings_rerun_setup), v -> {
            prefs.edit().putBoolean(PREF_SETUP_DONE, false).apply();
            go(Section.SETUP);
        }), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        View debugRow = Ui.buttonWithNote(this, getString(R.string.settings_debug_row),
                getString(R.string.settings_debug_note), v -> {
                    unlockDebug();
                    go(Section.DEBUG);
                });
        // The second, readable way in. It stays a row of Settings even before it is unlocked.
        launcher.addView(debugRow, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        screen.addView(launcher, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    /**
     * Settings, then Graphics driver: the system driver, every installed custom driver, and
     * the two actions. Everything it knows comes from {@link GpuDrivers}, which keeps the
     * drivers in the app's private storage; the game opens the chosen one before its renderer
     * starts (engine/nvf-android/src/gpu_driver.rs, docs/odin-debug.md).
     */
    private View driverRow() {
        LinearLayout panel = Ui.panel(this, getString(R.string.settings_driver_row));
        List<String[]> drivers = GpuDrivers.installed(this);
        String chosen = GpuDrivers.selected(this);
        panel.addView(Ui.switchRow(this, getString(R.string.settings_driver_system),
                getString(R.string.settings_driver_system_note), chosen == null, true, (b, on) -> {
                    GpuDrivers.select(this, null);
                    show();
                }));
        for (String[] driver : drivers) {
            final String id = driver[0];
            panel.addView(Ui.switchRow(this, driver[1], null, id.equals(chosen), true, (b, on) -> {
                GpuDrivers.select(this, on ? id : null);
                show();
            }), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
            panel.addView(Ui.button(this, getString(R.string.settings_driver_delete), v -> confirm(
                    getString(R.string.settings_driver_delete), () -> {
                        try {
                            GpuDrivers.delete(this, id);
                        } catch (RuntimeException e) {
                            toast(String.valueOf(e.getMessage()));
                        }
                        show();
                    })), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        }
        if (drivers.isEmpty()) {
            panel.addView(Ui.note(this, getString(R.string.settings_driver_none_note)),
                    Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 6, 0));
        }
        panel.addView(Ui.button(this, getString(R.string.settings_driver_install),
                v -> pickDriverZip()), Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        return panel;
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

    // ---- Debug -------------------------------------------------------------------------

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
        screen.addView(Ui.heading(this, getString(R.string.debug_title)));
        int on = ControlsFile.testsOn(controls).size();
        if (on > 0) {
            screen.addView(Ui.strip(this, getResources().getQuantityString(R.plurals.debug_warn_strip, on, on), Ui.WARN),
                    Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 8));
        }
        screen.addView(Ui.note(this, getString(R.string.debug_saves_untouched)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout engine = Ui.panel(this, getString(R.string.debug_group_engine));
        View first = testRow(engine, ControlsFile.SKIP_PIPBOY_WARMUP, R.string.debug_skip_pipboy,
                R.string.debug_skip_pipboy_note, true);
        defaultFocus = first;
        testRow(engine, ControlsFile.SKIP_VATS_OUTLINE, R.string.debug_skip_vats, R.string.debug_skip_vats_note, true);
        screen.addView(engine, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout vulkan = Ui.panel(this, getString(R.string.debug_group_vulkan));
        testRow(vulkan, ControlsFile.VULKAN_VALIDATION, R.string.debug_validation,
                R.string.debug_validation_note, true);
        // The two deeper checks are children of the layer: focusable, and inert while it is
        // off, which is the dependency the engine itself enforces.
        boolean layer = Boolean.TRUE.equals(controls.get(ControlsFile.VULKAN_VALIDATION));
        testRow(vulkan, ControlsFile.VULKAN_VALIDATION_SYNC, R.string.debug_validation_sync,
                R.string.debug_validation_sync_note, layer);
        testRow(vulkan, ControlsFile.VULKAN_VALIDATION_GPUAV, R.string.debug_validation_gpuav,
                R.string.debug_validation_gpuav_note, layer);
        testRow(vulkan, ControlsFile.GPU_BREADCRUMBS, R.string.debug_breadcrumbs,
                R.string.debug_breadcrumbs_note, true);
        // The chunks exist to be marked, so they are a child of the breadcrumbs exactly as the
        // two deeper checks are children of the layer; the engine enforces the same dependency.
        testRow(vulkan, ControlsFile.GPU_BREADCRUMB_CHUNKS, R.string.debug_breadcrumb_chunks,
                R.string.debug_breadcrumb_chunks_note,
                Boolean.TRUE.equals(controls.get(ControlsFile.GPU_BREADCRUMBS)));
        screen.addView(vulkan, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        // One switch a drawn feature, for narrowing down which of them the GPU dies in.
        LinearLayout drawing = Ui.panel(this, getString(R.string.debug_group_drawing));
        testRow(drawing, ControlsFile.NO_SOFT_PARTICLES, R.string.debug_no_soft_particles,
                R.string.debug_no_soft_particles_note, true);
        testRow(drawing, ControlsFile.NO_PARTICLES, R.string.debug_no_particles,
                R.string.debug_no_particles_note, true);
        testRow(drawing, ControlsFile.NO_DECALS, R.string.debug_no_decals,
                R.string.debug_no_decals_note, true);
        testRow(drawing, ControlsFile.NO_HEAT_HAZE, R.string.debug_no_heat_haze,
                R.string.debug_no_heat_haze_note, true);
        screen.addView(drawing, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout diagnostics = Ui.panel(this, getString(R.string.debug_group_diagnostics));
        testRow(diagnostics, ControlsFile.DIAGNOSTICS, R.string.debug_diagnostics,
                R.string.debug_diagnostics_note, true);
        screen.addView(diagnostics, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout reports = Ui.panel(this, getString(R.string.debug_group_reports));
        reports.addView(Ui.note(this, getString(R.string.debug_privacy)));
        reports.addView(Ui.button(this, getString(R.string.debug_export_log), v -> chooseDebugLogDestination()),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        reports.addView(Ui.button(this, getString(R.string.debug_copy_summary), v -> copyDebugSummary()),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        screen.addView(reports, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout reset = Ui.panel(this, getString(R.string.debug_group_reset));
        reset.addView(Ui.button(this, getString(R.string.debug_tests_off), v -> {
            ControlsFile.clearTests(controls);
            saveControls();
            toast(getString(R.string.debug_tests_off_done));
            show();
        }));
        screen.addView(reset, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View testRow(LinearLayout into, String key, int title, int note, boolean enabled) {
        View row = Ui.switchRow(this, getString(title), getString(note),
                Boolean.TRUE.equals(controls.get(key)), enabled, (b, on) -> {
                    setControl(key, on);
                    show();
                });
        into.addView(row, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        return row;
    }

    // ---- About -------------------------------------------------------------------------

    private View aboutScreen() {
        LinearLayout screen = Ui.column(this);
        screen.addView(Ui.heading(this, getString(R.string.about_title)));
        LinearLayout panel = Ui.panel(this, null);
        panel.addView(Ui.body(this, getString(R.string.about_disclaimer)));
        panel.addView(Ui.note(this, getString(R.string.about_trademark)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        panel.addView(Ui.note(this, getString(R.string.about_extender)),
                Ui.params(this, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        panel.addView(Ui.readout(this, getString(R.string.about_build), buildChipText(), Ui.TEXT),
                Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
        screen.addView(panel, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 10));

        LinearLayout licences = Ui.panel(this, getString(R.string.about_licences));
        View first = licenceRow(licences, R.string.about_licence_nvforever, "gpl-3.0.txt");
        defaultFocus = first;
        licenceRow(licences, R.string.about_licence_validation, "vulkan-validation-apache-2.0.txt");
        licenceRow(licences, R.string.about_licence_font, "jetbrains-mono-ofl-1.1.txt");
        licenceRow(licences, R.string.about_licence_crates, "rust-crates.txt");
        screen.addView(licences, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return screen;
    }

    private View licenceRow(LinearLayout into, int title, String asset) {
        View row = Ui.buttonWithNote(this, getString(title), getString(R.string.about_licence_read),
                v -> showLicence(getString(title), asset));
        into.addView(row, Ui.params(this, ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, 0, 8, 0));
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
        TextView view = Ui.label(this, text, Ui.SMALL_SP, Ui.TEXT, false);
        int pad = Ui.dp(this, 12);
        view.setPaddingRelative(pad, pad, pad, pad);
        new AlertDialog.Builder(this)
                .setTitle(title)
                .setView(Ui.page(this, view))
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

    /** PLAY, or CONTINUE with {@code save}: one start's save, through files/android-continue.txt. */
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
