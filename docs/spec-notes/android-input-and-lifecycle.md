# Android physical input and lifecycle

- [documented] The public Xbox 360 Fallout: New Vegas manual, p. 2,
  [PDF mirror](https://device.report/m/df61014b29150201cb222cb61b0d2123320e13c39dd0793fc928e16b43b19419.pdf),
  assigns A/B/X/Y to activation/Pip-Boy/reload/jump, LT/RT to aim/attack, LB/RB
  to view/V.A.T.S., stick clicks to sneak/grab, Back/Start to wait/pause, and
  D-pad Up to ammunition. The manual's p. 6 describes Pip-Boy section navigation.
- [documented] Android's [controller input guide](https://developer.android.com/develop/ui/views/touch-and-input/game-controllers/controller-input)
  describes native key events, stick and hat axes, and alternative trigger axes.
  Android's semantic button codes are defined by
  [KeyEvent](https://developer.android.com/reference/android/view/KeyEvent).
- [observed] On the owner's RP5, Android 13, recording physical A/B/X/Y presses
  with `getevent -lt` and checking its installed keylayout confirmed Android
  A/B/X/Y in that order. The device exposes trigger values through BRAKE/GAS.
  The adapter preserves that semantic order and accepts either trigger axis pair.
- [observed] In the original M6 Android build, selecting Continue reached the
  engine's desktop handoff, whose attempted successor process aborted. The
  observation came from the app's own logs and live Android execution, not from
  reversing any Bethesda executable.
- [documented] Android's [NativeActivity](https://developer.android.com/reference/android/app/NativeActivity)
  hosts a native library through the activity lifecycle. The M6 shell already
  ends its game process when the activity finishes, because its Bevy/winit event
  loop is initialized once per process.

The Android lifecycle solution is an implementation decision: wait for the engine's
save writers, pass a private JSON restart request to the Java activity, and restart
through the launcher in a new game process. Arguments are consumed once; the
launcher validates PID, UID and process name before ending the old game process.
No public Intent supplies a PID or arbitrary arguments.

The input adapter keeps the engine's v0.6 menu actions and adds left-stick menu
navigation. Native focus loss resets held inputs. Touch visibility and controller
selection are separate startup options. None of these choices establish complete
console parity or guarantee a frame rate on Android.
