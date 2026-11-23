package org.nvforever.android;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.StateListDrawable;
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
 * The launcher's look, built in code: direction A, "Terminal". Amber on near-black, one
 * monospace family (JetBrains Mono, OFL-1.1) at two weights, bracketed section headings,
 * label and value readouts.
 *
 * <p>Nothing here reproduces the game's own interface: the palette, the brackets and the
 * geometric marks are generic terminal-interface vocabulary, and the typeface is neither the
 * game's nor a lookalike (LEGAL.md, and the launcher design, section 0).
 *
 * <p>Every control this builds is focusable and shows a 2dp amber ring when it has focus, so
 * a D-pad player can always see where they are. A view with a click listener already treats
 * the A button (D-pad centre and Enter) as a click, so no key handling is needed per row.
 */
final class Ui {
    static final int BG = 0xFF0A0A0B;
    static final int PANEL = 0xFF101012;
    static final int LINE = 0xFF2E2A20;
    static final int TEXT = 0xFFECE7DD;
    static final int WEAK = 0xFF9A9389;
    static final int AMBER = 0xFFF0A32A;
    static final int AMBER_DIM = 0xFFC08424;
    static final int AMBER_EDGE = 0xFFA8721D;
    static final int WARN = 0xFFFFB84D;
    static final int BAD = 0xFFE06A50;

    /** Body text. Monospace needs the upper end of the comfortable range on a 6 inch panel. */
    static final float BODY_SP = 17f;
    static final float SMALL_SP = 14f;
    static final float TITLE_SP = 22f;
    static final float HERO_SP = 34f;

    private Ui() {}

    static int dp(Context c, float value) {
        return Math.round(value * c.getResources().getDisplayMetrics().density);
    }

    private static Typeface regular;
    private static Typeface bold;

    /**
     * The bundled family, kept once: a screen builds a few dozen labels and every one of them
     * asks for it. {@code Resources.getFont} arrived in API 26, which is this app's minimum.
     */
    static Typeface mono(Context c, boolean wantBold) {
        if (regular == null) {
            regular = c.getResources().getFont(R.font.nvf_mono);
            bold = Typeface.create(regular, Typeface.BOLD);
        }
        return wantBold ? bold : regular;
    }

    static TextView label(Context c, CharSequence text, float sp, int colour, boolean bold) {
        TextView t = new TextView(c);
        t.setText(text);
        t.setTextColor(colour);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        t.setTypeface(mono(c, bold));
        t.setGravity(Gravity.START);
        return t;
    }

    static TextView body(Context c, CharSequence text) {
        return label(c, text, BODY_SP, TEXT, false);
    }

    static TextView note(Context c, CharSequence text) {
        return label(c, text, SMALL_SP, WEAK, false);
    }

    static TextView heading(Context c, CharSequence text) {
        TextView t = label(c, text, TITLE_SP, AMBER, true);
        t.setLetterSpacing(0.12f);
        return t;
    }

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

    /** A bordered box with a bracketed heading, the design's panel. */
    static LinearLayout panel(Context c, CharSequence title) {
        LinearLayout l = column(c);
        GradientDrawable box = new GradientDrawable();
        box.setColor(PANEL);
        box.setStroke(dp(c, 1), LINE);
        box.setCornerRadius(dp(c, 2));
        l.setBackground(box);
        int pad = dp(c, 12);
        l.setPaddingRelative(pad, pad, pad, pad);
        if (title != null) {
            TextView t = label(c, title, SMALL_SP, AMBER_DIM, true);
            t.setLetterSpacing(0.14f);
            l.addView(t, params(c, ViewGroup.LayoutParams.MATCH_PARENT, 0, 0, 0, 8));
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

    /**
     * The focus and pressed states every control shares: a 2dp amber ring and a faint amber
     * wash when focused, so focus never rests on fill alone.
     */
    static StateListDrawable focusable(Context c, int fill, int border) {
        GradientDrawable focused = new GradientDrawable();
        focused.setColor(blend(fill, AMBER, 0.10f));
        focused.setStroke(dp(c, 2), AMBER);
        focused.setCornerRadius(dp(c, 2));
        GradientDrawable rest = new GradientDrawable();
        rest.setColor(fill);
        rest.setStroke(dp(c, 1), border);
        rest.setCornerRadius(dp(c, 2));
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_focused}, focused);
        states.addState(new int[] {android.R.attr.state_pressed}, focused);
        states.addState(new int[0], rest);
        return states;
    }

    private static int blend(int base, int over, float amount) {
        return Color.rgb(
                Math.round(Color.red(base) * (1 - amount) + Color.red(over) * amount),
                Math.round(Color.green(base) * (1 - amount) + Color.green(over) * amount),
                Math.round(Color.blue(base) * (1 - amount) + Color.blue(over) * amount));
    }

    /** A plain control: one line of text, 56dp of touch target, focusable. */
    static TextView button(Context c, CharSequence text, View.OnClickListener action) {
        TextView t = label(c, text, BODY_SP, TEXT, false);
        t.setMinHeight(dp(c, 56));
        t.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
        int pad = dp(c, 12);
        t.setPaddingRelative(pad, pad / 2, pad, pad / 2);
        t.setBackground(focusable(c, PANEL, LINE));
        t.setFocusable(true);
        t.setOnClickListener(action);
        t.setContentDescription(text);
        return t;
    }

    /** A control with a one-line explanation under its label. Both are read out together. */
    static LinearLayout buttonWithNote(Context c, CharSequence text, CharSequence noteText, View.OnClickListener action) {
        LinearLayout l = column(c);
        l.setMinimumHeight(dp(c, 56));
        int pad = dp(c, 12);
        l.setPaddingRelative(pad, pad / 2, pad, pad / 2);
        l.setBackground(focusable(c, PANEL, LINE));
        l.setFocusable(true);
        l.setOnClickListener(action);
        l.addView(label(c, text, BODY_SP, TEXT, false));
        if (noteText != null) {
            l.addView(note(c, noteText));
        }
        l.setContentDescription(noteText == null ? text : text + ". " + noteText);
        return l;
    }

    /** The big control on Home. Filled in amber, or outlined when the panel should stay dark. */
    static LinearLayout hero(Context c, CharSequence title, CharSequence sub, View.OnClickListener action) {
        LinearLayout l = column(c);
        l.setGravity(Gravity.CENTER_VERTICAL);
        int pad = dp(c, 16);
        l.setPaddingRelative(pad, pad, pad, pad);
        GradientDrawable rest = new GradientDrawable();
        rest.setColor(PANEL);
        rest.setStroke(dp(c, 2), AMBER_EDGE);
        rest.setCornerRadius(dp(c, 2));
        GradientDrawable focused = new GradientDrawable();
        focused.setColor(blend(PANEL, AMBER, 0.16f));
        focused.setStroke(dp(c, 3), AMBER);
        focused.setCornerRadius(dp(c, 2));
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_focused}, focused);
        states.addState(new int[] {android.R.attr.state_pressed}, focused);
        states.addState(new int[0], rest);
        l.setBackground(states);
        l.setFocusable(true);
        l.setOnClickListener(action);
        TextView t = label(c, title, HERO_SP, AMBER, true);
        t.setLetterSpacing(0.08f);
        l.addView(t);
        if (sub != null) {
            TextView s = label(c, sub, SMALL_SP, TEXT, false);
            s.setMaxLines(2);
            l.addView(s);
        }
        l.setContentDescription(sub == null ? title : title + ". " + sub);
        return l;
    }

    /**
     * A switch with its one-line explanation. The whole row is the target, so the switch
     * itself is not focusable: one focus stop a row, and the explanation is read with it.
     */
    static LinearLayout switchRow(Context c, CharSequence title, CharSequence noteText, boolean on,
            boolean enabled, final CompoundButton.OnCheckedChangeListener change) {
        LinearLayout l = row(c);
        l.setMinimumHeight(dp(c, 56));
        int pad = dp(c, 12);
        l.setPaddingRelative(pad, pad / 2, pad, pad / 2);
        l.setBackground(focusable(c, PANEL, LINE));
        l.setFocusable(true);
        LinearLayout texts = column(c);
        texts.addView(label(c, title, BODY_SP, enabled ? TEXT : WEAK, false));
        if (noteText != null) {
            texts.addView(note(c, noteText));
        }
        LinearLayout.LayoutParams grow = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        l.addView(texts, grow);
        final Switch box = new Switch(c);
        box.setChecked(on);
        box.setFocusable(false);
        box.setClickable(false);
        box.setEnabled(enabled);
        l.addView(box);
        l.setOnClickListener(v -> {
            if (!enabled) {
                return;
            }
            box.setChecked(!box.isChecked());
            change.onCheckedChanged(box, box.isChecked());
        });
        l.setContentDescription(title + ". " + (on ? "on" : "off") + (noteText == null ? "" : ". " + noteText));
        return l;
    }

    /** A label and value readout: {@code Data folder   /sdcard/NVForever/Data}. */
    static LinearLayout readout(Context c, CharSequence key, CharSequence value, int valueColour) {
        LinearLayout l = row(c);
        l.setGravity(Gravity.TOP);
        TextView k = label(c, key, SMALL_SP, WEAK, false);
        LinearLayout.LayoutParams kp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 0.38f);
        l.addView(k, kp);
        TextView v = label(c, value, SMALL_SP, valueColour, false);
        LinearLayout.LayoutParams vp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 0.62f);
        vp.setMarginStart(dp(c, 8));
        l.addView(v, vp);
        l.setContentDescription(key + ": " + value);
        return l;
    }

    /** A whole screen: the page colour, and a scroll so any font scale still reaches the end. */
    static ScrollView page(Context c, View content) {
        ScrollView s = new ScrollView(c);
        s.setBackgroundColor(BG);
        s.setFillViewport(true);
        s.addView(content, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return s;
    }

    /** A strip that states something the player must not miss, in warning amber. */
    static TextView strip(Context c, CharSequence text, int colour) {
        TextView t = label(c, text, SMALL_SP, colour, true);
        int pad = dp(c, 10);
        t.setPaddingRelative(pad, pad, pad, pad);
        GradientDrawable box = new GradientDrawable();
        box.setColor(PANEL);
        box.setStroke(dp(c, 1), colour);
        box.setCornerRadius(dp(c, 2));
        t.setBackground(box);
        return t;
    }

    /** A tick, a warning or a cross, so no state is told by colour alone. */
    static String mark(boolean ok) {
        return ok ? "[ok]" : "[x]";
    }
}
