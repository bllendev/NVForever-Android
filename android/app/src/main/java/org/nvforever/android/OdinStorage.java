package org.nvforever.android;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;

/**
 * All files access, for the game folder this fork reads in place (docs/fork.md).
 *
 * <p>Upstream imports a copy of the player's game into app storage and needs no storage
 * permission at all. This fork opens the folder where it is, which from Android 11 on means
 * the "All files access" special permission: it cannot be granted from a prompt, only from a
 * system settings page the player has to visit. So this asks, says why, and sends them there;
 * nothing here grants anything by itself.
 *
 * <p>It asks only of an install that already reads in place ({@link OdinFork#readsInPlace}),
 * so a fresh install that is going to use upstream's importer is never asked for a permission
 * it does not need. Choosing the folder in the launcher, which is what would create that file
 * on a new device, is the next phase of this work
 * (.hermes/plans/android-base-phases.md).
 */
final class OdinStorage {
    /** Android 10's runtime permission request, where there is no all files access yet. */
    static final int REQUEST_LEGACY_READ = 11;

    private OdinStorage() {
    }

    /** Whether this process can open the game folder in shared storage. */
    static boolean granted(Activity activity) {
        if (Build.VERSION.SDK_INT >= 30) return Environment.isExternalStorageManager();
        return activity.checkSelfPermission(android.Manifest.permission.READ_EXTERNAL_STORAGE)
            == android.content.pm.PackageManager.PERMISSION_GRANTED;
    }

    /**
     * Ask for all files access, once per launcher start, when this install reads its game
     * folder in place and cannot currently open it.
     */
    static void promptIfNeeded(Activity activity) {
        if (!OdinFork.readsInPlace(activity.getFilesDir()) || granted(activity)) return;
        if (Build.VERSION.SDK_INT < 30) {
            activity.requestPermissions(new String[]{android.Manifest.permission.READ_EXTERNAL_STORAGE}, REQUEST_LEGACY_READ);
            return;
        }
        String folder = OdinFork.dataPath(activity.getFilesDir());
        new AlertDialog.Builder(activity)
            .setTitle("Allow access to your game folder")
            .setMessage("NVForever reads your game files where they are, at " + folder
                + ", and never copies or changes them. Android calls this \"All files access\"."
                + " Turn it on for NVForever on the next screen, then come back.")
            .setPositiveButton("Open settings", (dialog, which) -> openSettings(activity))
            .setNegativeButton("Not now", null)
            .show();
    }

    private static void openSettings(Activity activity) {
        // The page for this app, with the whole list as the fallback: a device without the
        // per-app page still has the list of apps that may have the permission.
        Intent[] tries = {
            new Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:" + activity.getPackageName())),
            new Intent(android.provider.Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION),
        };
        for (Intent intent : tries) {
            try {
                activity.startActivity(intent);
                return;
            } catch (android.content.ActivityNotFoundException ignored) {
                // Try the next one; the message below is the last word.
            }
        }
        new AlertDialog.Builder(activity).setTitle("Allow access to your game folder")
            .setMessage("This device has no settings page for all files access. Grant it to NVForever from"
                + " Android's own app settings, under Special app access.")
            .setPositiveButton("Close", null).show();
    }
}
