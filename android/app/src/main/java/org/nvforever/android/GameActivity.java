package org.nvforever.android;

import android.app.NativeActivity;

/**
 * The game: Android's NativeActivity loading libnvf_android.so (the lib_name in the manifest).
 * It runs in its own process (":game"), and that process is ended when the game closes, because
 * the Rust engine (Bevy/winit) can only start once per process: without this, opening the game a
 * second time without clearing the app from the recent apps would crash.
 */
public class GameActivity extends NativeActivity {
    /** The launcher's "Keep the screen awake while playing", passed with the start Intent. */
    public static final String EXTRA_KEEP_AWAKE = "keepAwake";

    private final android.os.Handler restartHandler = new android.os.Handler(android.os.Looper.getMainLooper());
    private final Runnable checkRestart = new Runnable() {
        @Override public void run() {
            java.io.File request = new java.io.File(engineHome(), "android-restart.json");
            if (request.isFile()) {
                android.content.Intent intent = new android.content.Intent(GameActivity.this, LauncherActivity.class);
                intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK | android.content.Intent.FLAG_ACTIVITY_CLEAR_TOP | android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP);
                intent.putExtra("resumeGame", true);
                startActivity(intent);
                // The launcher validates the private request and ends this process.
                return;
            }
            restartHandler.postDelayed(this, 200);
        }
    };

    /**
     * The active data profile's home, where the engine writes its restart requests: the
     * folder named in files/profile.txt, else the one every install has had. Read here rather
     * than passed in the Intent, because the engine in this process derives it the same way
     * (nvf-android/src/paths.rs) and the two must never disagree.
     */
    private java.io.File engineHome() {
        String home = Profiles.DEFAULT_HOME;
        try {
            for (String line : java.nio.file.Files.readAllLines(
                    new java.io.File(getFilesDir(), "profile.txt").toPath(),
                    java.nio.charset.StandardCharsets.UTF_8)) {
                String text = line.trim();
                // The same refusals as paths::profile_home_rel: inside our own storage only.
                if (!text.isEmpty() && !text.startsWith("/") && !text.startsWith("\\")
                        && !text.contains(":") && !text.contains("..")) {
                    home = text;
                    break;
                }
            }
        } catch (java.io.IOException | RuntimeException ignored) {
            // No profile chosen yet, or the file cannot be read: the default home.
        }
        return new java.io.File(getFilesDir(), home);
    }

    @Override protected void onCreate(android.os.Bundle state) {
        super.onCreate(state);
        if (getIntent() != null && getIntent().getBooleanExtra(EXTRA_KEEP_AWAKE, true)) {
            getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        }
        restartHandler.postDelayed(checkRestart, 200);
    }

    @Override
    protected void onDestroy() {
        restartHandler.removeCallbacks(checkRestart);
        super.onDestroy();
        if (isFinishing()) {
            android.os.Process.killProcess(android.os.Process.myPid());
        }
    }
}
