# Translating the Android launcher

The launcher's own words (buttons, labels, errors) live in Android string resources and
follow the phone's language with no code at all. This is a separate job from translating the
engine's own words, which is `lang/README.md` and `lang/<id>.toml`; the two never mix.

| Layer | What it is | Where it lives | Who translates it |
|---|---|---|---|
| The launcher's words | the screens in this app | `android/app/src/main/res/values*/strings.xml` | you, with this file |
| The engine's words | the settings page and a few messages | `lang/<id>.toml` | `lang/README.md` |
| The game's text | items, dialogue, quests | the player's own plugins | nobody: a localized install already shows it |
| Mod text | whatever a mod ships | that mod's plugin | the mod author |

So an English launcher with a Russian game is correct behaviour, not a bug.

## Adding a language

1. Copy `android/app/src/main/res/values/strings.xml` to
   `android/app/src/main/res/values-<code>/strings.xml`, where `<code>` is the ISO 639-1
   language code, optionally with a region as `values-pt-rBR` (note the `r`). Examples:
   `values-de`, `values-fr`, `values-pt-rBR`, `values-zh-rCN`.
2. Translate the text between the tags. Leave every `name=` attribute exactly as it is.
3. Keep every `%1$s`, `%2$d` and `\n`. They may be reordered to suit the language; they may
   not be dropped or renumbered wrongly. `android/tools/check_strings.py` fails the build if
   a key's set of placeholders does not match the English one.
4. Keep the `<plurals>` items your language needs and delete the ones it does not. Android
   picks by its own rules.
5. Escape an apostrophe as `\'` and a literal `%` as `%%`. An unescaped apostrophe is the
   most common build failure in a translated `strings.xml`.
6. Do not translate file names, paths, INI keys, `FalloutNV.esm`, flag names, or the name
   NVForever.
7. Do not copy any text from the original game or its manuals (`LEGAL.md`). Write your own
   words, exactly as `lang/README.md` asks for the engine's.
8. Leave a key out if you are unsure. English is the default and Android falls back key by
   key, so a half-finished file ships safely and a missing key is a correct result.
9. Use no em dash (U+2014) and no en dash (U+2013) as a dash. Commas, colons, parentheses
   and periods instead. The checker fails on either character.
10. Test with the device language set to yours, and with the system font size at 200%: that
    is where a long translation breaks a layout.

## Checking your work without an Android SDK

```sh
python android/tools/check_strings.py
```

It parses every `strings.xml`, reports a key a translation has that English does not, a
placeholder set that does not match, a `R.string.x` the Java uses that no file defines, and
any em dash or en dash.

## What is shipped today

`values/` (English, complete and required), `values-es/` and `values-pt-rBR/`. Those two
exist because the first Android testers wrote in those languages; the text was re-authored
from the new English rather than carried over, because the old strings described a screen
that no longer exists and two of them were wrong about the controller.
