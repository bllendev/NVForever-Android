package org.nvforever.android;

import android.app.NativeActivity;

/**
 * The game: Android's NativeActivity loading libnvf_android.so (the lib_name in the manifest).
 * It runs in its own process (":game"), and that process is ended when the game closes, because
 * the Rust engine (Bevy/winit) can only start once per process: without this, opening the game a
 * second time without clearing the app from the recent apps would crash.
 */
public class GameActivity extends NativeActivity {
    private final android.os.Handler restartHandler = new android.os.Handler(android.os.Looper.getMainLooper());
    private final Runnable checkRestart = new Runnable() {
        @Override public void run() {
            java.io.File request = new java.io.File(getFilesDir(), "NVForever/android-restart.json");
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

    @Override protected void onCreate(android.os.Bundle state) {
        super.onCreate(state);
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
