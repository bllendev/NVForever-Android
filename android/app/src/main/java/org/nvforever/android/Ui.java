package org.nvforever.android;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.LayerDrawable;
import android.graphics.drawable.StateListDrawable;
import android.text.TextUtils;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.CompoundButton;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Switch;
import android.widget.TextView;

/**
 * The launcher's look, built in code: design v2, direction A, "Main menu". A short
 * left-aligned vertical menu, warm amber on warm near-black, one highlight on the screen at a
 * time, and a slim status strip at the foot of every screen.
 *
 * <p>Two families. Inter (SIL OFL 1.1) carries everything a person reads; JetBrains Mono (SIL
 * OFL 1.1) is kept for technical values alone: the version, paths, file names, sizes and the
 * {@code [ok]} and {@code [x]} marks. Both are bundled and both are named in About.
 *
 * <p>Nothing here reproduces the game's own interface. A left vertical text menu, dark
 * negative space and one warm highlight are generic menu vocabulary; the typefaces are neither
 * the game's nor lookalikes, there is no game art, mark or wording (LEGAL.md).
 *
 * <p>Focus is one model everywhere: a 3dp amber bar on the row's left edge plus a faint wash
 * that fades out to the right. Never a box per item, and never fill alone. A view with a click
 * listener already treats the A button (D-pad centre and Enter) as a click, so no key handling
 * is needed per row.
 */
final class Ui {
    // ---- palette: one accent, everything else warm neutral -----------------------------

    static final int BG = 0xFF0B0A08;
    static final int BG_2 = 0xFF100E0B;
    static final int RULE = 0xFF221E18;
    static final int RULE_2 = 0xFF2E281F;
    static final int INK = 0xFFEDE6D9;
    static final int INK_2 = 0xFFA79F92;
    static final int INK_3 = 0xFF6E675D;
    static final int AMBER = 0xFFE8A33D;
    static final int AMBER_MID = 0xFFB57C26;
    static final int AMBER_DEEP = 0xFF6B4812;
    static final int AMBER_WASH = 0xFF1A1308;
    /** The same wash with no alpha, so the fade stays warm instead of going grey. */
    static final int AMBER_WASH_CLEAR = 0x001A1308;
    static final int WARN = 0xFFD9A441;
    static final int BAD = 0xFFC9604A;
    /** The unchecked switch track: a neutral grey, never the platform teal. */
    static final int SWITCH_OFF = 0xFF332D25;

    // ---- type scale, sp ----------------------------------------------------------------

    /** Version, paths, counts, file sizes, the controller legend. */
    static final float MICRO_SP = 11f;
    /** The second line of a row, readout values, notes. */
    static final float SMALL_SP = 13f;
    /** Row labels, and the default. */
    static final float BODY_SP = 16f;
    /** The screen name and the detail panel's title. */
    static final float LEAD_SP = 18f;
    /** The main menu items. */
    static final float MENU_SP = 20f;
    /** Exactly one item a screen. */
    static final float HERO_SP = 26f;
    /** Small-caps group headers. */
    static final float HEAD_SP = 12f;

    // ---- spacing, dp -------------------------------------------------------------------

    /** The screen's side margin. */
    static final int GUTTER_DP = 36;
    /** A settings or menu row: the touch and focus target. */
    static final int ROW_DP = 48;
    /** A dense row, used where a column holds six of them. */
    static final int ROW_DENSE_DP = 44;
    /** The focus bar on the left edge of the focused row. */
    static final int BAR_DP = 3;

    private Ui() {}

    static int dp(Context c, float value) {
        return Math.round(value * c.getResources().getDisplayMetrics().density);
    }

    // ---- the two families --------------------------------------------------------------

    private static Typeface sansRegular;
    private static Typeface sansBold;
    private static Typeface monoRegular;
    private static Typeface monoBold;

    /**
     * Inter, the proportional face everything readable is set in. Kept once: a screen builds a
     * few dozen labels and every one of them asks for it. {@code Resources.getFont} arrived in
     * API 26, which is this app's minimum.
     */
    static Typeface sans(Context c, boolean wantBold) {
        if (sansRegular == null) {
            sansRegular = c.getResources().getFont(R.font.nvf_sans);
            sansBold = Typeface.create(sansRegular, Typeface.BOLD);
        }
        return wantBold ? sansBold : sansRegular;
    }

    /** JetBrains Mono, for technical values only. */
    static Typeface mono(Context c, boolean wantBold) {
        if (monoRegular == null) {
            monoRegular = c.getResources().getFont(R.font.nvf_mono);
            monoBold = Typeface.create(monoRegular, Typeface.BOLD);
        }
        return wantBold ? monoBold : monoRegular;
    }

    // ---- text --------------------------------------------------------------------------

    static TextView label(Context c, CharSequence text, float sp, int colour, boolean bold) {
        TextView t = new TextView(c);
        t.setText(text);
        t.setTextColor(colour);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        t.setTypeface(sans(c, bold));
        t.setGravity(Gravity.START);
        return t;
    }

    /** A technical value: a version, a path, a file name, a size, a mark. */
    static TextView technical(Context c, CharSequence text, float sp, int colour) {
        TextView t = label(c, text, sp, colour, false);
        t.setTypeface(mono(c, false));
        return t;
    }

    static TextView body(Context c, CharSequence text) {
        return label(c, text, BODY_SP, INK, false);
    }

    static TextView note(Context c, CharSequence text) {
        TextView t = label(c, text, SMALL_SP, INK_3, false);
        t.setLineSpacing(0f, 1.2f);
        return t;
    }

    static TextView micro(Context c, CharSequence text, int colour) {
        return technical(c, text, MICRO_SP, colour);
    }

    /** A quiet small-caps group header: the only structure a screen draws. */
    static TextView groupHead(Context c, CharSequence text) {
        TextView t = label(c, text, HEAD_SP, INK_3, false);
        t.setAllCaps(true);
        t.setLetterSpacing(0.18f);
        return t;
    }

    /** The name of the screen you are on, in the one accent. */
    static TextView screenTitle(Context c, CharSequence text) {
        return label(c, text, LEAD_SP, AMBER, true);
    }

    /** The line beside a screen name that says what the screen is for. */
    static TextView crumb(Context c, CharSequence text) {
        return label(c, text, SMALL_SP, INK_3, false);
    }

    // ---- containers ---------------------------------------------------------------------

    static LinearLayout column(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.VERTICAL);
        return l;
    }

    static LinearLayout row(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.HORIZONTAL);
        l.setGravity(Gravity.CENTER_VERTICAL);
        return l;
    }

    /**
     * A list of rows: hairlines above, between and below, and no fill and no outline anywhere.
     * The hairline is the whole frame a row gets when it is not focused.
     */
    static LinearLayout list(Context c) {
        LinearLayout l = column(c);
        GradientDrawable line = new GradientDrawable();
        line.setColor(RULE);
        line.setSize(0, 1);
        l.setDividerDrawable(line);
        l.setShowDividers(LinearLayout.SHOW_DIVIDER_BEGINNING | LinearLayout.SHOW_DIVIDER_MIDDLE
                | LinearLayout.SHOW_DIVIDER_END);
        return l;
    }

    /** A group: a small-caps header and its list of rows. */
    static LinearLayout group(Context c, CharSequence head) {
        LinearLayout l = column(c);
        if (head != null) {
            l.addView(groupHead(c, head), params(c, ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT, 0, 0, 6));
        }
        return l;
    }

    /** Layout parameters with margins in dp, start and end aware. */
    static LinearLayout.LayoutParams params(Context c, int width, int height, int startDp, int topDp, int bottomDp) {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(width, height);
        p.setMarginStart(dp(c, startDp));
        p.topMargin = dp(c, topDp);
        p.bottomMargin = dp(c, bottomDp);
        return p;
    }

    static LinearLayout.LayoutParams wide(Context c, int bottomDp) {
        return params(c, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT,
                0, 0, bottomDp);
    }

    /** One hairline, where a list is not what is being drawn. */
    static View hairline(Context c) {
        View v = new View(c);
        v.setBackgroundColor(RULE);
        return v;
    }

    // ---- the page -----------------------------------------------------------------------

    /**
     * The page itself: warm near-black, two soft washes, and one abstract horizon hairline
     * low on the screen with a breath of warmth under it. Vector only, nothing from the game.
     */
    static Drawable background(Context c) {
        ColorDrawable base = new ColorDrawable(BG);

        GradientDrawable warm = new GradientDrawable();
        warm.setShape(GradientDrawable.RECTANGLE);
        warm.setGradientType(GradientDrawable.RADIAL_GRADIENT);
        warm.setColors(new int[] {AMBER_WASH, AMBER_WASH_CLEAR});
        warm.setGradientCenter(0.82f, 1.10f);
        warm.setGradientRadius(dp(c, 420));

        GradientDrawable cool = new GradientDrawable();
        cool.setShape(GradientDrawable.RECTANGLE);
        cool.setGradientType(GradientDrawable.RADIAL_GRADIENT);
        cool.setColors(new int[] {BG_2, 0x00100E0B});
        cool.setGradientCenter(0.10f, -0.10f);
        cool.setGradientRadius(dp(c, 300));

        GradientDrawable horizon = new GradientDrawable(GradientDrawable.Orientation.LEFT_RIGHT,
                new int[] {0x00E8A33D, 0x1FE8A33D, 0x00E8A33D});

        LayerDrawable page = new LayerDrawable(new Drawable[] {base, cool, warm, horizon});
        page.setLayerGravity(3, Gravity.FILL_HORIZONTAL | Gravity.BOTTOM);
        page.setLayerSize(3, -1, 1);
        page.setLayerInsetBottom(3, dp(c, 72));
        return page;
    }

    /** A whole screen: a scroll, so any system font scale still reaches the end. */
    static ScrollView page(Context c, View content) {
        ScrollView s = new ScrollView(c);
        s.setFillViewport(true);
        s.addView(content, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return s;
    }

    // ---- the focus model ------------------------------------------------------------------

    /** The focused row's fill: a warm wash fading to nothing to the right. Nothing at rest. */
    private static StateListDrawable wash() {
        GradientDrawable focused = new GradientDrawable(GradientDrawable.Orientation.LEFT_RIGHT,
                new int[] {AMBER_WASH, AMBER_WASH_CLEAR});
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_focused}, focused);
        states.addState(new int[] {android.R.attr.state_pressed}, focused);
        states.addState(new int[0], new ColorDrawable(0x00000000));
        return states;
    }

    /** The 3dp bar on the focused row's left edge. It follows its row's state. */
    private static View focusBar(Context c) {
        View bar = new View(c);
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_focused}, new ColorDrawable(AMBER));
        states.addState(new int[] {android.R.attr.state_pressed}, new ColorDrawable(AMBER));
        states.addState(new int[0], new ColorDrawable(0x00000000));
        bar.setBackground(states);
        bar.setDuplicateParentStateEnabled(true);
        return bar;
    }

    /**
     * The shell every focusable row shares: the bar, the wash and the height. The caller fills
     * {@code inner}, which is the row's second child and the only one that holds content.
     */
    private static LinearLayout shell(Context c, int heightDp, int padStartDp) {
        LinearLayout outer = row(c);
        outer.setMinimumHeight(dp(c, heightDp));
        outer.setBackground(wash());
        outer.addView(focusBar(c), new LinearLayout.LayoutParams(
                dp(c, BAR_DP), ViewGroup.LayoutParams.MATCH_PARENT));
        LinearLayout inner = row(c);
        inner.setPaddingRelative(dp(c, padStartDp), dp(c, 4), dp(c, 12), dp(c, 4));
        outer.addView(inner, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        return outer;
    }

    private static LinearLayout innerOf(LinearLayout shell) {
        return (LinearLayout) shell.getChildAt(1);
    }

    // ---- rows ------------------------------------------------------------------------------

    /**
     * One row: a label, an optional second line, an optional leading mark and an optional
     * trailing view. 48dp by default, 44dp dense. A disabled row keeps its focus stop, so a
     * D-pad player can read why it is greyed instead of skipping over it.
     */
    static LinearLayout row(Context c, CharSequence title, CharSequence second, View leading,
            View trailing, int heightDp, boolean enabled, View.OnClickListener action) {
        return row(c, title, second, false, leading, trailing, heightDp, enabled, action);
    }

    /** The same row, with its second line set in the technical face: a path, a size, a count. */
    static LinearLayout row(Context c, CharSequence title, CharSequence second, boolean secondTechnical,
            View leading, View trailing, int heightDp, boolean enabled, View.OnClickListener action) {
        LinearLayout outer = shell(c, heightDp, 12);
        LinearLayout inner = innerOf(outer);
        if (leading != null) {
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                    dp(c, 16), dp(c, 16));
            lp.setMarginEnd(dp(c, 12));
            inner.addView(leading, lp);
        }
        LinearLayout texts = column(c);
        TextView label = label(c, title, BODY_SP, enabled ? INK : INK_3, false);
        label.setSingleLine(true);
        label.setEllipsize(TextUtils.TruncateAt.END);
        texts.addView(label);
        if (second != null) {
            TextView sub = secondTechnical ? technical(c, second, MICRO_SP, INK_3)
                    : label(c, second, SMALL_SP, INK_3, false);
            sub.setSingleLine(true);
            sub.setEllipsize(TextUtils.TruncateAt.MIDDLE);
            texts.addView(sub);
        }
        inner.addView(texts, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        if (trailing != null) {
            LinearLayout.LayoutParams tp = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            tp.setMarginStart(dp(c, 12));
            inner.addView(trailing, tp);
        }
        outer.setFocusable(true);
        if (action != null) {
            outer.setOnClickListener(v -> {
                if (enabled) {
                    action.onClick(v);
                }
            });
        }
        outer.setContentDescription(second == null ? title : title + ". " + second);
        return outer;
    }

    /** A row that leads somewhere: the chevron says so. */
    static LinearLayout chevronRow(Context c, CharSequence title, CharSequence second, int heightDp,
            View.OnClickListener action) {
        TextView chevron = label(c, ">", BODY_SP, INK_3, false);
        return row(c, title, second, null, chevron, heightDp, true, action);
    }

    /** A row whose second line is a path: technical, and ellipsised in the middle. */
    static LinearLayout pathRow(Context c, CharSequence title, CharSequence path, int heightDp,
            View.OnClickListener action) {
        TextView chevron = label(c, ">", BODY_SP, INK_3, false);
        return row(c, title, path, true, null, chevron, heightDp, true, action);
    }

    /** A row that states a value it does not change here. */
    static LinearLayout valueRow(Context c, CharSequence title, CharSequence second, CharSequence value,
            boolean technical, int heightDp, View.OnClickListener action) {
        TextView v = technical ? technical(c, value, MICRO_SP, INK_3) : label(c, value, SMALL_SP, INK_2, false);
        return row(c, title, second, null, v, heightDp, true, action);
    }

    /**
     * A switch at the end of its row, 32dp by 18dp of track, tinted in the one accent. The
     * whole row is the target, so the switch itself is not focusable: one focus stop a row, and
     * the explanation is read with it.
     */
    static LinearLayout switchRow(Context c, CharSequence title, CharSequence second, boolean on,
            boolean enabled, int heightDp, final CompoundButton.OnCheckedChangeListener change) {
        final Switch box = new Switch(c);
        box.setChecked(on);
        box.setFocusable(false);
        box.setClickable(false);
        box.setEnabled(enabled);
        box.setTrackTintList(new ColorStateList(
                new int[][] {{android.R.attr.state_checked}, {}},
                new int[] {AMBER_DEEP, SWITCH_OFF}));
        box.setThumbTintList(new ColorStateList(
                new int[][] {{android.R.attr.state_checked}, {}},
                new int[] {AMBER, INK_3}));
        LinearLayout r = row(c, title, second, null, box, heightDp, enabled, v -> {
            box.setChecked(!box.isChecked());
            change.onCheckedChanged(box, box.isChecked());
        });
        r.setContentDescription(title + ". " + (on ? "on" : "off") + (second == null ? "" : ". " + second));
        return r;
    }

    /** One option of an exclusive choice: a 16dp ring, filled when it is the chosen one. */
    static LinearLayout radioRow(Context c, CharSequence title, CharSequence second, boolean chosen,
            int heightDp, View.OnClickListener action) {
        View ring = new View(c);
        GradientDrawable mark = new GradientDrawable();
        mark.setShape(GradientDrawable.OVAL);
        mark.setColor(0x00000000);
        mark.setStroke(dp(c, 2), chosen ? AMBER : INK_3);
        if (chosen) {
            GradientDrawable dot = new GradientDrawable();
            dot.setShape(GradientDrawable.OVAL);
            dot.setColor(AMBER);
            LayerDrawable both = new LayerDrawable(new Drawable[] {mark, dot});
            both.setLayerInset(1, dp(c, 4), dp(c, 4), dp(c, 4), dp(c, 4));
            ring.setBackground(both);
        } else {
            ring.setBackground(mark);
        }
        LinearLayout r = row(c, title, second, ring, null, heightDp, true, action);
        r.setContentDescription(title + ". " + (chosen ? "chosen" : "not chosen")
                + (second == null ? "" : ". " + second));
        return r;
    }

    /** The empty leading slot, so a plain row lines up with the radio rows above it. */
    static View indent(Context c) {
        return new View(c);
    }

    /**
     * A main-menu item: a short left-aligned line, the one hero among them larger. Same focus
     * model as every other row, with more room around it.
     */
    static LinearLayout menuItem(Context c, CharSequence title, CharSequence hint, boolean isHero,
            View.OnClickListener action) {
        LinearLayout outer = shell(c, isHero ? 56 : 42, 15);
        LinearLayout inner = innerOf(outer);
        inner.setGravity(Gravity.CENTER_VERTICAL);
        TextView label = label(c, title, isHero ? HERO_SP : MENU_SP, INK_2, isHero);
        label.setTextColor(new ColorStateList(
                new int[][] {{android.R.attr.state_focused}, {android.R.attr.state_pressed}, {}},
                new int[] {AMBER, AMBER, INK_2}));
        label.setDuplicateParentStateEnabled(true);
        inner.addView(label);
        if (hint != null) {
            TextView h = label(c, hint, SMALL_SP, INK_3, false);
            LinearLayout.LayoutParams hp = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            hp.setMarginStart(dp(c, 10));
            inner.addView(h, hp);
        }
        outer.setFocusable(true);
        outer.setOnClickListener(action);
        outer.setContentDescription(hint == null ? title : title + ". " + hint);
        return outer;
    }

    // ---- the footers -------------------------------------------------------------------

    /** The full-width strip of facts at the foot of every screen. */
    static LinearLayout statusStrip(Context c) {
        LinearLayout strip = row(c);
        strip.setMinimumHeight(dp(c, 32));
        strip.setPaddingRelative(0, dp(c, 6), 0, 0);
        LayerDrawable top = new LayerDrawable(new Drawable[] {new ColorDrawable(RULE)});
        top.setLayerGravity(0, Gravity.FILL_HORIZONTAL | Gravity.TOP);
        top.setLayerSize(0, -1, 1);
        strip.setBackground(top);
        return strip;
    }

    /** One fact on the strip: a small-caps key and its value. */
    static void cell(LinearLayout strip, Context c, CharSequence key, CharSequence value,
            boolean technical, int colour) {
        if (strip.getChildCount() > 0) {
            View sep = new View(c);
            sep.setBackgroundColor(RULE_2);
            LinearLayout.LayoutParams sp = new LinearLayout.LayoutParams(1, dp(c, 12));
            sp.setMarginStart(dp(c, 14));
            sp.setMarginEnd(dp(c, 14));
            strip.addView(sep, sp);
        }
        LinearLayout cell = row(c);
        TextView k = label(c, key, MICRO_SP, INK_3, false);
        k.setAllCaps(true);
        k.setLetterSpacing(0.14f);
        cell.addView(k);
        TextView v = technical ? technical(c, value, MICRO_SP, colour) : label(c, value, SMALL_SP, colour, false);
        v.setSingleLine(true);
        v.setEllipsize(TextUtils.TruncateAt.MIDDLE);
        LinearLayout.LayoutParams vp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        vp.setMarginStart(dp(c, 5));
        cell.addView(v, vp);
        cell.setContentDescription(key + ": " + value);
        strip.addView(cell);
    }

    /** The controller legend, the last line of every screen. */
    static LinearLayout legend(Context c) {
        LinearLayout l = row(c);
        l.setMinimumHeight(dp(c, 28));
        return l;
    }

    /** One legend entry: the button in a rounded outline, then what it does. */
    static void legendItem(LinearLayout legend, Context c, CharSequence button, CharSequence what) {
        LinearLayout item = row(c);
        if (legend.getChildCount() > 0) {
            item.setPaddingRelative(dp(c, 20), 0, 0, 0);
        }
        TextView glyph = label(c, button, MICRO_SP, AMBER_MID, true);
        glyph.setPaddingRelative(dp(c, 5), dp(c, 1), dp(c, 5), dp(c, 1));
        GradientDrawable ring = new GradientDrawable();
        ring.setStroke(dp(c, 1), AMBER_DEEP);
        ring.setCornerRadius(dp(c, 8));
        glyph.setBackground(ring);
        item.addView(glyph);
        TextView text = label(c, what, MICRO_SP, INK_2, false);
        LinearLayout.LayoutParams tp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        tp.setMarginStart(dp(c, 5));
        item.addView(text, tp);
        legend.addView(item);
    }

    /** The quiet sentence at the end of the legend line. */
    static void legendHint(LinearLayout legend, Context c, CharSequence text) {
        TextView t = label(c, text, MICRO_SP, INK_3, false);
        t.setGravity(Gravity.END);
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(0,
                ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        p.setMarginStart(dp(c, 16));
        legend.addView(t, p);
    }

    /**
     * A band that states something the player must not miss: a bar in the warning colour on
     * its left edge and a very faint wash behind it. No box, like everything else.
     */
    static LinearLayout band(Context c, CharSequence text, int colour) {
        LinearLayout l = row(c);
        l.setMinimumHeight(dp(c, 36));
        l.setBackgroundColor(0x12D9A441);
        View bar = new View(c);
        bar.setBackgroundColor(colour);
        l.addView(bar, new LinearLayout.LayoutParams(dp(c, BAR_DP), ViewGroup.LayoutParams.MATCH_PARENT));
        TextView mark = technical(c, "[!]", MICRO_SP, colour);
        mark.setPaddingRelative(dp(c, 11), 0, dp(c, 8), 0);
        l.addView(mark);
        TextView t = label(c, text, SMALL_SP, colour, false);
        l.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        l.setPaddingRelative(0, 0, dp(c, 12), 0);
        l.setContentDescription(text);
        return l;
    }

    /** A tick or a cross, so no state is told by colour alone. */
    static String mark(boolean ok) {
        return ok ? "[ok]" : "[x]";
    }
}
