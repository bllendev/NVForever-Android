package org.nvforever.android;

import android.content.Context;

/**
 * What the debug report says about {@link GpuDrivers}: which Vulkan driver the next game start
 * uses, and why the last one did not use what was chosen. Choosing, installing and deleting a
 * driver are the launcher's own Settings, then Graphics driver row; this class adds no UI and
 * no state of its own.
 */
final class GpuDriversDebug {
    private GpuDriversDebug() {}

    /** The one line the debug report carries: which driver the next start will use. */
    static String summary(Context context) {
        String id = GpuDrivers.selected(context);
        String label = "System";
        if (id != null) {
            label = id;
            for (String[] driver : GpuDrivers.installed(context)) {
                if (driver[0].equals(id)) label = driver[1];
            }
        }
        String fellBack = GpuDrivers.lastStartFellBack(context);
        return "GPU driver: " + label + (fellBack == null ? "" : "\nLast start fell back: " + fellBack);
    }
}
