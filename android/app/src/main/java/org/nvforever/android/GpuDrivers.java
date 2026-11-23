package org.nvforever.android;

import android.content.Context;
import android.net.Uri;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/**
 * Custom Vulkan drivers the player installed (Turnip through libadrenotools), and which one
 * the game starts with.
 *
 * <p><b>This is the agreed contract and a stub body.</b> The work that loads a driver is on
 * another branch; it replaces the bodies below and nothing else. The launcher's Settings,
 * then Graphics driver row, and Home's fall-back message are written against these six
 * methods only, so the two sides can land in either order.
 *
 * <p>The contract:
 * <ul>
 * <li>{@link #installed} lists the drivers that are ready to use, each as a two-element
 *     array {@code {id, label}}: {@code id} is stable and is what {@link #select} is given,
 *     {@code label} is what the row shows ("Turnip 25.1.0 (Adreno 7xx)"). The list is empty
 *     when none is installed, never null, and the System driver is not in it.</li>
 * <li>{@link #selected} is the chosen driver's id, or null for the system driver. An id that
 *     is no longer installed reads as null.</li>
 * <li>{@link #select} chooses one, or the system driver with null. It applies at the next
 *     game start, like every other launcher switch.</li>
 * <li>{@link #install} takes a driver zip the player picked, unpacks it and returns its new
 *     id. It throws {@link IOException} with a message fit to show when the zip is not a
 *     driver, is for another GPU, or cannot be read.</li>
 * <li>{@link #delete} removes an installed driver. Selecting it afterwards is the system
 *     driver. Deleting an id that is not installed does nothing.</li>
 * <li>{@link #lastStartFellBack} is null when the last game start used what was chosen, and
 *     a short reason ("the driver did not load") when it fell back to the system driver, so
 *     Home can say so instead of the player wondering why nothing changed. Reading it does
 *     not clear it; it is replaced by the next start.</li>
 * </ul>
 *
 * <p>No method here ever touches a save, a plugin list or a Data folder.
 */
public final class GpuDrivers {
    private GpuDrivers() {}

    /** The installed drivers as {@code {id, label}}. Empty while this is a stub. */
    public static List<String[]> installed(Context c) {
        return new ArrayList<>();
    }

    /** The chosen driver's id, or null for the system driver. Always null while this is a stub. */
    public static String selected(Context c) {
        return null;
    }

    /** Chooses a driver, or the system driver with null. Does nothing while this is a stub. */
    public static void select(Context c, String idOrNull) {
        // The real body stores the choice; the game reads it at the next start.
    }

    /** Unpacks a driver zip and returns its id. */
    public static String install(Context c, Uri zip) throws IOException {
        throw new UnsupportedOperationException(
                "Installing a graphics driver is not built into this version yet.");
    }

    /** Removes an installed driver. Does nothing while this is a stub. */
    public static void delete(Context c, String id) {
        throw new UnsupportedOperationException(
                "Installing a graphics driver is not built into this version yet.");
    }

    /** Why the last start fell back to the system driver, or null when it did not. */
    public static String lastStartFellBack(Context c) {
        return null;
    }
}
