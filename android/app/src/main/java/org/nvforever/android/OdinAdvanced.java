package org.nvforever.android;

import android.app.Activity;
import android.app.AlertDialog;
import android.graphics.Typeface;
import android.view.Gravity;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.io.File;
import java.io.IOException;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * The fork's Advanced section: the crash-test switches, hidden behind a long press on the
 * build line of the launcher's Home page (docs/odin-debug.md).
 *
 * <p>It is hidden because none of these switches is a setting: each one leaves a drawn feature
 * out of the game, or turns on a diagnostic that makes the game slower, to narrow down why the
 * owner's Odin 3 loses its graphics device. A player who has not been asked to use one should
 * never find one by scrolling.
 *
 * <p>It is reachable because the owner has to use it on the device, with no PC and no
 * {@code adb}: one long press on the build line of Home, and the page says in the same place
 * how many switches are on, so nobody plays for a week wondering why the game stutters.
 *
 * <p>What a row means and what the file holds is {@link OdinCrashTests}, which is plain Java
 * and tested; this class is the page that shows the rows. The engine reads the same file at
 * start ({@code engine/nvforever/src/odin/crash_tests.rs}).
 *
 * <p>Upstream's launcher is touched in one place only (docs/fork.md): {@link #attach} on the
 * build line of {@code MobileLauncher.homePage}.
 */
final class OdinAdvanced {
    /** The amber the launcher's own theme uses for anything that wants the eye. */
    private static final int AMBER = 0xFFE8B44A;

    /** What the build line says when nothing is on: the long press is not advertised. */
    private static final int WEAK = 0xFF9A9289;

    private OdinAdvanced() {
    }

    /**
     * Say on {@code build} how many crash tests are on, and open this page when it is long
     * pressed. {@code home} is the engine home, where the switches are kept.
     */
    static void attach(Activity activity, TextView build, File home) {
        CharSequence words = build.getText();
        say(build, words, home);
        build.setLongClickable(true);
        build.setOnLongClickListener(v -> {
            show(activity, home, () -> say(build, words, home));
            return true;
        });
    }

    /** {@code build} reading {@code words}, with the count of what is on after it. */
    private static void say(TextView build, CharSequence words, File home) {
        String summary = OdinCrashTests.summary(OdinCrashTests.read(home));
        build.setText(summary.isEmpty() ? words : words + "\n" + summary + " (long press)");
        build.setTextColor(summary.isEmpty() ? WEAK : AMBER);
    }

    /**
     * The page itself: one row per switch, with what it does under its label, and one action
     * that turns every one of them off. Nothing is applied to a running game: the switches are
     * read when the engine next starts, like every other launcher choice.
     */
    static void show(Activity activity, File home, Runnable changed) {
        Set<String> on = OdinCrashTests.settled(OdinCrashTests.read(home));
        LinearLayout list = new LinearLayout(activity);
        list.setOrientation(LinearLayout.VERTICAL);
        int pad = dp(activity, 16);
        list.setPadding(pad, dp(activity, 8), pad, dp(activity, 8));
        TextView why = new TextView(activity);
        why.setText("Each switch leaves one thing out of the game, or turns on a diagnostic that makes it slower, so a crash can be narrowed down. They take effect the next time the game starts. Leave them all off to play.");
        why.setTextSize(14);
        why.setTextColor(WEAK);
        list.addView(why);
        if (GpuDrivers.mode(activity.getFilesDir()) != null) {
            // Validation and a driver the player supplied are mutually exclusive: the layer
            // chain under libadrenotools' private namespace is not something this port has
            // checked, so the engine stays on the system driver whenever validation is on
            // (odin::gpu_driver, docs/odin-drivers.md).
            TextView driver = new TextView(activity);
            driver.setText("A graphics driver of your own is chosen in Settings. While Vulkan validation is on, the next start uses the system driver instead and says so in the launcher.");
            driver.setTextSize(14);
            driver.setTextColor(AMBER);
            list.addView(driver);
        }
        // Every row is rebuilt whenever one changes, because a row may need another switch and
        // is greyed until that one is on.
        Runnable[] rebuild = new Runnable[1];
        LinearLayout rows = new LinearLayout(activity);
        rows.setOrientation(LinearLayout.VERTICAL);
        list.addView(rows);
        rebuild[0] = () -> {
            rows.removeAllViews();
            for (OdinCrashTests.Switch row : OdinCrashTests.SWITCHES) {
                boolean usable = OdinCrashTests.usable(row, on);
                CheckBox box = new CheckBox(activity);
                box.setText(row.label);
                box.setTextSize(16);
                box.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
                box.setChecked(on.contains(row.key));
                box.setEnabled(usable);
                box.setOnClickListener(v -> {
                    if (box.isChecked()) {
                        on.add(row.key);
                    } else {
                        on.remove(row.key);
                    }
                    Set<String> settled = OdinCrashTests.settled(on);
                    on.retainAll(settled);
                    save(activity, home, on, changed);
                    rebuild[0].run();
                });
                rows.addView(box);
                TextView note = new TextView(activity);
                note.setText(row.needs == null || usable ? row.note
                    : row.note + " Needs " + name(row.needs) + ".");
                note.setTextSize(13);
                note.setTextColor(WEAK);
                note.setPadding(dp(activity, 36), 0, 0, dp(activity, 10));
                rows.addView(note);
            }
        };
        rebuild[0].run();
        ScrollView scroll = new ScrollView(activity);
        scroll.addView(list);
        AlertDialog dialog = new AlertDialog.Builder(activity)
            .setTitle("Advanced: crash tests")
            .setView(scroll)
            .setPositiveButton("Done", null)
            .create();
        dialog.show();
        // Not a dialog button: "turn everything off" has to stay visible while the rows are
        // being read, and a neutral button would sit beside Done as though it were a choice
        // between the two.
        Button clear = new Button(activity);
        clear.setAllCaps(false);
        clear.setText("Turn every crash test off");
        clear.setOnClickListener(v -> {
            on.clear();
            save(activity, home, on, changed);
            rebuild[0].run();
        });
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-1, -2);
        lp.topMargin = dp(activity, 8);
        clear.setGravity(Gravity.CENTER);
        list.addView(clear, lp);
    }

    /** Write the switches, and say so on the build line; a failure is said, never silent. */
    private static void save(Activity activity, File home, Set<String> on, Runnable changed) {
        try {
            OdinCrashTests.write(home, new LinkedHashSet<>(on));
        } catch (IOException | RuntimeException e) {
            new AlertDialog.Builder(activity)
                .setTitle("Advanced: crash tests")
                .setMessage("The switches could not be saved: " + e)
                .setPositiveButton("Close", null)
                .show();
            return;
        }
        changed.run();
    }

    /** What a row the player has to turn on first is called, for the note that says so. */
    private static String name(String key) {
        OdinCrashTests.Switch row = OdinCrashTests.find(key);
        return row == null ? key : row.label;
    }

    private static int dp(Activity activity, float value) {
        return Math.round(value * activity.getResources().getDisplayMetrics().density);
    }
}
