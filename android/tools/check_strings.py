"""Checks the launcher's string resources without an Android SDK.

Run from the repository root: `python android/tools/check_strings.py`.

1. Every res/values*/strings.xml parses as XML.
2. Every `R.string.x` and `R.plurals.x` the Java uses exists in res/values/strings.xml
   (the default file, the only one that has to be complete).
3. No translation names a key the English file does not have (a typo there falls back
   silently and is never seen).
4. Every string with a `%1$s`-style placeholder has the same set of placeholders in every
   language, so a translation cannot crash `getString` with a missing argument.
5. No em dash (U+2014) and no en dash (U+2013) anywhere (house style).
"""

import glob
import os
import re
import sys
import xml.dom.minidom

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RES = os.path.join(ROOT, "android", "app", "src", "main", "res")
JAVA = os.path.join(ROOT, "android", "app", "src", "main", "java")
PLACEHOLDER = re.compile(r"%(\d+)\$[sd]")
DASHES = re.compile("[" + chr(0x2014) + chr(0x2013) + "]")


def names(path):
    """The string and plurals names in one strings.xml, and their placeholders."""
    doc = xml.dom.minidom.parse(path)
    found = {}
    for tag in ("string", "plurals"):
        for node in doc.getElementsByTagName(tag):
            name = node.getAttribute("name")
            text = "".join(n.toxml() for n in node.childNodes)
            found[name] = set(PLACEHOLDER.findall(text))
    return found


def main():
    problems = []
    files = sorted(glob.glob(os.path.join(RES, "values*", "strings.xml")))
    if not files:
        return fail(["no strings.xml found under " + RES])
    default = os.path.join(RES, "values", "strings.xml")
    english = names(default)
    print("%d keys in the default file" % len(english))

    for path in files:
        if path == default:
            continue
        other = names(path)
        for key, marks in other.items():
            if key not in english:
                problems.append("%s: %s is not in the default file" % (rel(path), key))
            elif marks != english[key]:
                problems.append("%s: %s has placeholders %s, the default has %s"
                                % (rel(path), key, sorted(marks) or "none", sorted(english[key]) or "none"))
        print("%s: %d keys" % (rel(path), len(other)))

    used = set()
    for root, _, found in os.walk(JAVA):
        for name in found:
            if not name.endswith(".java"):
                continue
            with open(os.path.join(root, name), encoding="utf-8") as handle:
                text = handle.read()
            used |= set(re.findall(r"R\.(?:string|plurals)\.([A-Za-z0-9_]+)", text))
    for key in sorted(used):
        if key not in english:
            problems.append("the Java uses R.string.%s, which no strings.xml defines" % key)
    print("%d keys used by the Java" % len(used))

    for path in files + [os.path.join(ROOT, "android", "app", "src", "main", "AndroidManifest.xml")]:
        with open(path, encoding="utf-8") as handle:
            for number, line in enumerate(handle, 1):
                # Named by code point so this file itself stays clean of both characters.
                if DASHES.search(line):
                    problems.append("%s:%d: an em dash or en dash" % (rel(path), number))

    # aapt2 rejects a bare apostrophe or double quote inside a string ("Invalid unicode
    # escape sequence"), which plain XML parsing and javac never notice.
    element = re.compile(r"<(string|item)\b[^>]*>(.*?)</\1>", re.S)
    for path in files:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        for match in element.finditer(text):
            if re.search(r"(?<!\\)['\"]", match.group(2)):
                number = text.count("\n", 0, match.start()) + 1
                problems.append("%s:%d: an unescaped ' or \" (write \\' or \\\")" % (rel(path), number))

    return fail(problems)


def rel(path):
    return os.path.relpath(path, ROOT).replace("\\", "/")


def fail(problems):
    for problem in problems:
        print("FAIL " + problem)
    if problems:
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
