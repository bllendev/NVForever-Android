package org.nvforever.android;

import java.io.File;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.io.StringWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

final class DebugReport {
    private static final Pattern RUN = Pattern.compile("nvforever-(\\d{8}-\\d{6})-(\\d+)\\.log");
    private static final Pattern CRASH = Pattern.compile("crash-(\\d{8}-\\d{6})-(\\d+)\\.txt");
    // The file-layer warning emitted by crash::report_start, not a generic mention of a log.
    private static final Pattern PREVIOUS = Pattern.compile(
            "(?m)^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(?:\\.\\d+)?Z +WARN nvforever::crash: "
            + Pattern.quote("run log: the previous run did not end normally and left no crash report "
                    + "(a native crash, or the process was ended): ") + "([^\\x0d\\x0a]+)\\x0d?\\x0a",
            Pattern.UNIX_LINES);
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
        String reference = append(latest, out, true);
        Matcher run = RUN.matcher(latest.getName());
        if (!run.matches()) return;
        File crash = null;
        for (File file : files) {
            Matcher m = CRASH.matcher(file.getName());
            if (file.isFile() && m.matches() && m.group(2).equals(run.group(2))
                    && m.group(1).compareTo(run.group(1)) >= 0
                    && (crash == null || file.getName().compareTo(crash.getName()) > 0)) crash = file;
        }
        if (crash != null) append(crash, out, false);
        if (reference != null) appendPrevious(directory, reference, run.group(1), out);
    }

    private static void appendPrevious(File directory, String reference, String latestStamp, Writer out)
            throws IOException {
        File named = new File(reference);
        String name = named.getName();
        Matcher run = RUN.matcher(name);
        if (!named.isAbsolute() || !run.matches() || run.group(1).compareTo(latestStamp) >= 0) return;

        StringWriter evidence = new StringWriter();
        try {
            // Open only a direct child resolved from the trusted logs directory, never the
            // path from the warning. Also allow Android's canonical alias for the directory.
            File candidate = new File(directory.getAbsoluteFile(), name);
            if (!named.equals(candidate) && !named.equals(new File(directory.getCanonicalFile(), name))) return;
            if (!Files.isRegularFile(candidate.toPath(), LinkOption.NOFOLLOW_LINKS)) return;
            append(candidate, evidence, false);
        } catch (IOException | SecurityException ignored) {
            // The optional log may have been rotated away. Keep the primary export intact.
            return;
        }
        out.write("\nPrevious abnormal run (referenced by the latest run)\n");
        out.write(evidence.toString());
    }

    private static String append(File file, Writer out, boolean findPrevious) throws IOException {
        out.write("\n=== " + file.getName() + " ===\n");
        String reference;
        try (RandomAccessFile input = new RandomAccessFile(file, "r")) {
            long length = input.length();
            boolean truncated = length > HEAD + TAIL;
            String start = readBytes(input, truncated ? HEAD : (int) length);
            out.write(start);
            reference = findPrevious ? previousReference(start, 0) : null;
            if (truncated) {
                out.write("\n[Middle of log omitted]\n");
                input.seek(length - TAIL - 1);
                boolean startsOnNewLine = input.read() == '\n';
                String end = readBytes(input, TAIL);
                out.write(end);
                if (findPrevious && reference == null) {
                    // The first tail line may begin in the omitted middle; don't trust it.
                    int firstNewline = end.indexOf('\n');
                    if (startsOnNewLine || firstNewline >= 0) {
                        reference = previousReference(end, startsOnNewLine ? 0 : firstNewline + 1);
                    }
                }
            }
        }
        out.write("\n");
        return reference;
    }

    private static String previousReference(String text, int start) {
        Matcher warning = PREVIOUS.matcher(text);
        return warning.find(start) ? warning.group(1) : null;
    }

    private static String readBytes(RandomAccessFile input, int size) throws IOException {
        byte[] bytes = new byte[size];
        input.readFully(bytes);
        return new String(bytes, StandardCharsets.UTF_8);
    }
}
