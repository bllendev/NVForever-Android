package org.nvforever.android;

/**
 * The compact density the launcher draws itself at on a handheld (docs/fork.md).
 *
 * <p>Upstream's Android launcher is drawn for a phone held upright: full-width buttons 48 dp
 * tall, 12 dp padding, 19 sp card titles and one card under another. On the owner's AYN Odin 3
 * (1920x1080, about six inches, landscape, sticks and buttons) that fills the screen with four
 * rows and pushes everything else under the fold, which is how a crash left him with the Report
 * a problem card where he could not see it (his words, 2026-11-27: "the menu is too bulky and
 * shows very little").
 *
 * <p>So on a landscape screen that is small for its pixels, or on any screen with a physical
 * gamepad attached, the same launcher is drawn at a compact density: text one or two steps
 * down, rows 44 dp (which is still a touch target), padding and gaps about seven tenths, and
 * the cards in two or three columns where the width allows. Upstream's palette, its words and
 * its pages are untouched; only the numbers change.
 *
 * <p>Nothing here is a scale factor applied blindly: the text sizes come from a table so that
 * a title stays a title and a note stays readable, and {@link #rowHeight()} never goes under
 * 44 dp whatever else does.
 *
 * <p>Plain Java, with no Android types, so it is tested with javac and java alone
 * (.github/workflows/fork-android.yml, the controls job).
 */
final class OdinCompact {
    /** Under this smallest width in dp, a landscape screen is a handheld's, not a tablet's. */
    static final int HANDHELD_SW_DP = 600;

    /** The smallest touch target the compact density may produce, in dp. */
    static final int TOUCH_TARGET_DP = 44;

    /** Two columns of cards need at least this much width in dp. */
    static final int TWO_COLUMN_DP = 640;

    /** Three columns need at least this much. */
    static final int THREE_COLUMN_DP = 900;

    /** Two columns of plugin rows, which are narrower than a card, need this much. */
    static final int TWO_PLUGIN_COLUMNS_DP = 720;

    /** What padding, margins and fixed boxes are multiplied by. */
    private static final float SPACE = 0.7f;

    /** Whether this launcher is drawn compact at all. */
    final boolean on;

    private OdinCompact(boolean on) {
        this.on = on;
    }

    /**
     * The density for a screen: compact in landscape on a screen whose smallest side is
     * under {@link #HANDHELD_SW_DP}, and compact on any screen with a gamepad attached,
     * because a device with sticks is a device being held.
     */
    static OdinCompact of(boolean landscape, int smallestWidthDp, boolean gamepad) {
        return new OdinCompact(gamepad || (landscape && smallestWidthDp < HANDHELD_SW_DP));
    }

    /** Upstream's own density, for a phone held upright with no controller. */
    static OdinCompact roomy() {
        return new OdinCompact(false);
    }

    /**
     * A text size in sp: upstream's, or the compact step down from it. A title stays a
     * title (22 to 18, 19 to 16), body text stays readable (15 to 14) and nothing small
     * gets smaller, because the countdown's digits are already at the floor.
     */
    int text(int size) {
        if (!on) {
            return size;
        }
        if (size >= 22) {
            return 18;
        }
        if (size >= 19) {
            return 16;
        }
        if (size >= 17) {
            return 15;
        }
        if (size >= 15) {
            return 14;
        }
        if (size >= 13) {
            return 12;
        }
        return size;
    }

    /** A padding, margin or fixed box in dp, compacted but never under 2 dp. */
    int scale(int dp) {
        return on ? Math.max(2, Math.round(dp * SPACE)) : dp;
    }

    /** The same, named for the place it is used most. */
    int pad(int dp) {
        return scale(dp);
    }

    /** How tall a row or a button is, which stays a touch target however compact it is. */
    int rowHeight() {
        return on ? TOUCH_TARGET_DP : 48;
    }

    /** The gap between one card or row and the next. */
    int gap() {
        return on ? 4 : 8;
    }

    /** How many columns of cards a screen this wide holds. */
    int columns(int widthDp) {
        if (!on) {
            return 1;
        }
        if (widthDp >= THREE_COLUMN_DP) {
            return 3;
        }
        return widthDp >= TWO_COLUMN_DP ? 2 : 1;
    }

    /** How many columns the plugin list is dealt into, in load order down each one. */
    int pluginColumns(int widthDp) {
        return on && widthDp >= TWO_PLUGIN_COLUMNS_DP ? 2 : 1;
    }

    /**
     * How many of {@code count} rows go in the first of {@code columns} columns, so that the
     * load order reads down one column and then down the next rather than zigzagging.
     */
    static int firstColumn(int count, int columns) {
        if (columns <= 1) {
            return count;
        }
        return (count + columns - 1) / columns;
    }
}
