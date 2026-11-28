package org.nvforever.android;

/** JDK-only checks of the compact density's numbers; no Android device required. */
public final class OdinCompactTest {
    private static int checks;

    private static void same(Object actual, Object expected, String what) {
        checks++;
        if (!expected.equals(actual)) throw new AssertionError(what + ": " + actual + " != " + expected);
    }

    private static void yes(boolean value, String what) {
        checks++;
        if (!value) throw new AssertionError(what);
    }

    /** The screens the launcher meets: the owner's handheld, a phone, a tablet. */
    private static void whenItIsCompact() {
        // The owner's Odin 3: 1920x1080 at density 2 is 960x540 dp, landscape, with sticks.
        yes(OdinCompact.of(true, 540, true).on, "a handheld in landscape with a controller");
        yes(OdinCompact.of(true, 540, false).on, "the same handheld with the controller asleep");
        // A phone held upright is upstream's own case and is left alone.
        yes(!OdinCompact.of(false, 392, false).on, "a phone upright stays upstream's");
        // A tablet in landscape has the room, so it keeps the roomy density.
        yes(!OdinCompact.of(true, 800, false).on, "a tablet in landscape stays upstream's");
        // Buttons attached mean a device being held, whichever way round it is.
        yes(OdinCompact.of(false, 392, true).on, "a gamepad is enough on its own");
        yes(!OdinCompact.roomy().on, "and the roomy density is never compact");
    }

    /** Text: a title stays a title, body text stays readable, small text does not shrink. */
    private static void theTextSizes() {
        OdinCompact compact = OdinCompact.of(true, 540, true);
        same(compact.text(22), 18, "the launcher's own name");
        same(compact.text(19), 16, "a card title");
        same(compact.text(17), 15, "a settings heading");
        same(compact.text(16), 14, "body text");
        same(compact.text(15), 14, "body text");
        same(compact.text(14), 12, "a note");
        same(compact.text(12), 12, "the save preview's detail is already small");
        same(compact.text(9), 9, "and the countdown's header is at the floor");
        for (int size = 9; size <= 24; size++) {
            yes(compact.text(size) <= size, "no text grows: " + size);
            yes(compact.text(size) >= 9, "and none disappears: " + size);
            yes(OdinCompact.roomy().text(size) == size, "upstream's sizes are untouched: " + size);
        }
        for (int size = 10; size <= 24; size++) {
            yes(compact.text(size) >= compact.text(size - 1), "the order of sizes is kept at " + size);
        }
    }

    /** Space: padding shrinks, rows stay touch targets, upstream's numbers are untouched. */
    private static void theSpacing() {
        OdinCompact compact = OdinCompact.of(true, 540, true);
        same(compact.pad(12), 8, "a card's padding");
        same(compact.pad(8), 6, "a bar's padding");
        same(compact.pad(4), 3, "a margin");
        same(compact.pad(2), 2, "and nothing goes under two");
        same(compact.scale(36), 25, "the header icon");
        same(compact.gap(), 4, "the gap between cards");
        yes(compact.rowHeight() >= OdinCompact.TOUCH_TARGET_DP, "a row is still a touch target");
        yes(compact.rowHeight() < 48, "and it is shorter than upstream's");
        same(OdinCompact.roomy().rowHeight(), 48, "upstream's row height");
        same(OdinCompact.roomy().pad(12), 12, "upstream's padding");
        same(OdinCompact.roomy().gap(), 8, "upstream's gap");
        for (int dp = 1; dp <= 64; dp++) {
            yes(compact.scale(dp) <= Math.max(2, dp), "nothing grows: " + dp);
            yes(compact.scale(dp) >= 2, "and nothing vanishes: " + dp);
        }
    }

    /** Columns: how many cards fit side by side, and how the plugin list is dealt. */
    private static void theColumns() {
        OdinCompact compact = OdinCompact.of(true, 540, true);
        same(compact.columns(960), 3, "the owner's screen holds three columns of cards");
        same(compact.columns(720), 2, "a narrower one holds two");
        same(compact.columns(480), 1, "and a narrow one holds one");
        same(OdinCompact.roomy().columns(960), 1, "upstream's launcher is one column");
        same(compact.pluginColumns(960), 2, "plugins are dealt into two columns");
        same(compact.pluginColumns(600), 1, "but not on a narrow screen");
        same(OdinCompact.roomy().pluginColumns(960), 1, "and never in upstream's density");

        // Load order reads down the first column and then down the second.
        same(OdinCompact.firstColumn(12, 2), 6, "twelve plugins split in half");
        same(OdinCompact.firstColumn(13, 2), 7, "an odd one goes in the first column");
        same(OdinCompact.firstColumn(1, 2), 1, "one plugin fills the first column");
        same(OdinCompact.firstColumn(0, 2), 0, "and none fills nothing");
        same(OdinCompact.firstColumn(12, 1), 12, "one column holds them all");
        for (int count = 0; count <= 40; count++) {
            int first = OdinCompact.firstColumn(count, 2);
            yes(first >= count - first, "the first column is never the shorter: " + count);
            yes(count - first >= 0, "and the second is never negative: " + count);
        }
    }

    public static void main(String[] args) {
        whenItIsCompact();
        theTextSizes();
        theSpacing();
        theColumns();
        System.out.println("OdinCompactTest: " + checks + " checks passed");
    }
}
