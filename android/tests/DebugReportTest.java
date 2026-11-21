package org.nvforever.android;

import java.io.File;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
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
        } finally {
            try (var paths = Files.walk(dir)) {
                for (Path path : (Iterable<Path>) paths.sorted(Comparator.reverseOrder())::iterator) {
                    Files.delete(path);
                }
            }
        }
        System.out.println("DebugReport tests passed");
    }

    private static void check(boolean condition, String name) {
        if (!condition) throw new AssertionError(name);
    }
}
