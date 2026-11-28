package org.nvforever.android;

import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;

public final class DebugReportTest {
    public static void main(String[] args) throws Exception {
        Path dir = Files.createTempDirectory("nvf-report-test");
        try {
            StringWriter out = new StringWriter();
            DebugReport.write(dir.toFile(), "build and device", out);
            check(out.toString().contains("No engine run log"), "empty state");
            Files.writeString(dir.resolve("nvforever-20261119-120000-21.log"), "OLD RUN");
            Files.writeString(dir.resolve("crash-20261119-120001-21.txt"), "OLD CRASH");
            Files.writeString(dir.resolve("crash-20261120-120001-42.txt"), "CURRENT CRASH");
            Files.writeString(dir.resolve("crash-20261119-120002-42.txt"), "STALE SAME PID");
            Files.writeString(dir.resolve("NVForever.ini"), "PRIVATE CONFIG");
            Files.writeString(dir.resolve("nvforever-20261120-120000-42.log"),
                    "START OF RUN\n" + "x".repeat(300_000) + "\nEND OF RUN");
            out = new StringWriter();
            DebugReport.write(dir.toFile(), "build and device", out);
            String report = out.toString();
            check(report.contains("build and device"), "build and device header");
            check(report.contains("START OF RUN") && report.contains("END OF RUN"), "keeps start and tail");
            check(report.length() < 300_000 && report.contains("omitted"), "bounded export");
            check(report.contains("CURRENT CRASH"), "current-run crash report");
            for (String excluded : new String[]{"OLD CRASH", "OLD RUN", "STALE SAME PID", "PRIVATE CONFIG"}) {
                check(!report.contains(excluded), "exclude " + excluded);
            }
            testStartLogs(Files.createDirectory(dir.resolve("starts")));
            testReferencedPriorRun(Files.createDirectory(dir.resolve("prior")));
            testRejectedReferences(Files.createDirectory(dir.resolve("rejected")));
            testTruncatedReference(Files.createDirectory(dir.resolve("truncated")));
        } finally {
            try (var paths = Files.walk(dir)) {
                for (Path path : (Iterable<Path>) paths.sorted(Comparator.reverseOrder())::iterator) {
                    Files.delete(path);
                }
            }
        }
        System.out.println("DebugReport tests passed");
    }

    /**
     * The fork's start log (odin/start_log.rs): what the engine printed before the run log
     * was open. A start that is refused leaves only that, which is exactly the case the
     * export used to come back empty for.
     */
    private static void testStartLogs(Path dir) throws Exception {
        Files.writeString(dir.resolve("start-20261127-120000-42.txt"),
                "NVForever start log\nerror: no game folder\n");
        String report = export(dir);
        check(report.contains("No engine run log"), "says there is no run log");
        check(report.contains("error: no game folder"), "a refused start still exports what it printed");

        // A run that opened a run log exports the start log its own process wrote.
        Files.writeString(dir.resolve("nvforever-20261127-130000-43.log"), "THE RUN\n");
        Files.writeString(dir.resolve("start-20261127-130000-43.txt"), "THIS RUN'S START\n");
        report = export(dir);
        check(report.contains("THE RUN") && report.contains("THIS RUN'S START"), "both halves of one run");
        check(!report.contains("error: no game folder"), "and not an older start log");

        // A start after the newest run log is one that never got a run log of its own.
        Files.writeString(dir.resolve("start-20261127-140000-44.txt"), "THE REFUSED START\n");
        report = export(dir);
        check(report.contains("THE REFUSED START"), "the newest start log is exported when it is newer than the run");
        check(report.contains("THE RUN"), "and the newest run log is still there");
    }

    private static void testReferencedPriorRun(Path dir) throws Exception {
        Path prior = dir.resolve("nvforever-20261120-120000-42.log");
        Path latest = dir.resolve("nvforever-20261120-120100-43.log");
        Files.writeString(prior, "PRIOR START\n" + "x".repeat(300_000) + "\nPRIOR END\n");
        Files.writeString(dir.resolve("nvforever-20261120-120050-99.log"), "UNREFERENCED RUN");
        Files.writeString(latest, warning(prior) + "LATEST CLEAN RUN\n");
        String report = export(dir);
        check(report.contains("PRIOR START") && report.contains("PRIOR END"), "keeps referenced prior evidence");
        check(report.contains("Previous abnormal run") && report.indexOf("LATEST CLEAN RUN") < report.indexOf("PRIOR START"),
                "labels prior evidence separately from the latest run");
        check(report.length() < 240_000 && report.contains("[Middle of log omitted]"), "prior evidence stays bounded");
        check(!report.contains("UNREFERENCED RUN"), "does not infer prior run from file ordering");

        Files.writeString(dir.resolve("crash-20261120-120101-43.txt"), "LATEST MATCHED CRASH");
        Files.writeString(dir.resolve("crash-20261120-120059-43.txt"), "STALE SAME-PID CRASH");
        Files.writeString(dir.resolve("crash-20261120-120102-42.txt"), "UNRELATED CRASH");
        report = export(dir);
        check(report.contains("LATEST MATCHED CRASH") && report.contains("PRIOR END"),
                "prior evidence does not displace the latest run's crash report");
        check(!report.contains("STALE SAME-PID CRASH") && !report.contains("UNRELATED CRASH"),
                "crash matching still uses the latest run's PID and timestamp");
    }

    private static void testRejectedReferences(Path dir) throws Exception {
        Path prior = dir.resolve("nvforever-20261120-120000-42.log");
        Path latest = dir.resolve("nvforever-20261120-120100-43.log");
        Path sameTime = dir.resolve("nvforever-20261120-120100-41.log");
        Path config = dir.resolve("NVForever.ini");
        Path outside = dir.getParent().resolve(prior.getFileName());
        Files.writeString(prior, "LOCAL PRIOR");
        Files.writeString(sameTime, "SAME TIME");
        Files.writeString(config, "PRIVATE CONFIG");
        Files.writeString(outside, "OUTSIDE CONTENT");
        for (Path rejected : new Path[]{dir.resolve("nvforever-20261120-115959-42.log"), outside,
                dir.resolve("..").resolve(prior.getFileName()), dir.resolve(".").resolve(prior.getFileName()),
                prior.getFileName(), latest, sameTime, config}) {
            Files.writeString(latest, warning(rejected) + "LATEST STILL EXPORTED\n");
            String report = export(dir);
            check(report.contains("LATEST STILL EXPORTED"), "keeps latest when reference rejected: " + rejected);
            check(!report.contains("Previous abnormal run"), "rejects reference: " + rejected);
        }
        Files.writeString(latest, warning(prior).replace("nvforever::crash:", "other::module:"));
        check(!export(dir).contains("LOCAL PRIOR"), "rejects another log source quoting the warning");
    }

    private static void testTruncatedReference(Path dir) throws Exception {
        Path prior = dir.resolve("nvforever-20261120-120000-42.log");
        Path latest = dir.resolve("nvforever-20261120-120100-43.log");
        Files.writeString(prior, "PRIOR EVIDENCE");
        String warning = warning(prior);
        String tail = warning + "x".repeat(192 * 1024 - warning.getBytes(StandardCharsets.UTF_8).length);
        Files.writeString(latest, "x".repeat(300_000) + "\n" + tail);
        check(export(dir).contains("PRIOR EVIDENCE"), "accepts complete warning at retained tail boundary");
        Files.writeString(latest, "x".repeat(300_000) + tail);
        check(!export(dir).contains("PRIOR EVIDENCE"), "rejects warning-looking suffix of a truncated line");
    }

    private static String warning(Path path) {
        return "2026-11-20T12:01:00.000000Z  WARN nvforever::crash: run log: the previous run did not end normally"
                + " and left no crash report (a native crash, or the process was ended): " + path + "\n";
    }

    private static String export(Path dir) throws Exception {
        StringWriter out = new StringWriter();
        DebugReport.write(dir.toFile(), "build and device", out);
        return out.toString();
    }

    private static void check(boolean condition, String name) {
        if (!condition) throw new AssertionError(name);
    }
}
