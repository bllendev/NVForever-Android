"""Writes a stand-in R.java from the launcher's resources, for an offline javac check.

The Android build generates R from the resources with aapt2. On a machine with no SDK,
`javac -bootclasspath android<N>.jar` can still compile every launcher source as long as
something called `org.nvforever.android.R` exists with the right names in it. That is all
this writes: a class of int constants whose values mean nothing.

    python android/tools/gen_stub_r.py <path to R.java>

It is only a compile aid. The real build and CI use aapt2 and never read this.
"""

import os
import sys
import xml.dom.minidom

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RES = os.path.join(ROOT, "android", "app", "src", "main", "res")


def names(path, tag):
    doc = xml.dom.minidom.parse(path)
    return [node.getAttribute("name") for node in doc.getElementsByTagName(tag)]


def main(out):
    strings_xml = os.path.join(RES, "values", "strings.xml")
    fonts = sorted(
        os.path.splitext(name)[0]
        for name in os.listdir(os.path.join(RES, "font"))
        if name.endswith(".xml")
    )
    lines = [
        "package org.nvforever.android;",
        "",
        "/** Stand-in for the aapt-generated R, for the offline javac check only. */",
        "public final class R {",
    ]
    for kind, tag, base in (("string", "string", 0x7F010000), ("plurals", "plurals", 0x7F020000)):
        lines.append("    public static final class %s {" % kind)
        for n, name in enumerate(names(strings_xml, tag)):
            lines.append("        public static final int %s = 0x%08x;" % (name, base + n))
        lines.append("    }")
    lines.append("    public static final class font {")
    for n, name in enumerate(fonts):
        lines.append("        public static final int %s = 0x%08x;" % (name, 0x7F030000 + n))
    lines.append("    }")
    lines.append("}")
    with open(out, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")
    print("wrote %s" % out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
