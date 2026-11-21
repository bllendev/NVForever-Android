package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

final class DebugReport {
    private static final Pattern RUN = Pattern.compile("nvforever-(\\d{8}-\\d{6})-(\\d+)\\.log");
    private static final Pattern CRASH = Pattern.compile("crash-(\\d{8}-\\d{6})-(\\d+)\\.txt");
    private static final int HEAD = 32 * 1024;
    private static final int TAIL = 192 * 1024;

    private DebugReport() {}

    static void write(File directory, String header, Writer out) throws IOException {
        out.write(header + "\nReview before sharing; logs may contain local paths and mod names.\n");
        File[] files = directory.listFiles();
        File latest = null;
        if (files != null) {
            for (File file : files) {
                if (file.isFile() && RUN.matcher(file.getName()).matches()
                        && (latest == null || file.getName().compareTo(latest.getName()) > 0)) latest = file;
            }
        }
        if (latest == null) {
            out.write("\nNo engine run log yet.\n");
            return;
        }
        append(latest, out);
        Matcher run = RUN.matcher(latest.getName());
        if (!run.matches()) return;
        File crash = null;
        for (File file : files) {
            Matcher m = CRASH.matcher(file.getName());
            if (file.isFile() && m.matches() && m.group(2).equals(run.group(2))
                    && m.group(1).compareTo(run.group(1)) >= 0
                    && (crash == null || file.getName().compareTo(crash.getName()) > 0)) crash = file;
        }
        if (crash != null) append(crash, out);
    }

    private static void append(File file, Writer out) throws IOException {
        out.write("\n=== " + file.getName() + " ===\n");
        try (RandomAccessFile input = new RandomAccessFile(file, "r")) {
            long length = input.length();
            if (length <= HEAD + TAIL) {
                writeBytes(input, (int) length, out);
            } else {
                writeBytes(input, HEAD, out);
                out.write("\n[Middle of log omitted]\n");
                input.seek(length - TAIL);
                writeBytes(input, TAIL, out);
            }
        }
        out.write("\n");
    }

    private static void writeBytes(RandomAccessFile input, int size, Writer out) throws IOException {
        byte[] bytes = new byte[size];
        input.readFully(bytes);
        out.write(new String(bytes, StandardCharsets.UTF_8));
    }
}
