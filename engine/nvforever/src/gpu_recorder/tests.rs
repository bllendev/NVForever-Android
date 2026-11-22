//! The ring and the report formatting, without a GPU.

use super::report::{self, Sampled};
use super::ring::{feature, feature_text, Frame, Ring, FRAMES};

/// A record with the fields a test cares about.
fn frame(index: u64, frame_us: u32) -> Frame {
    Frame { index, frame_us, ..Frame::EMPTY }
}

#[test]
fn an_empty_ring_holds_nothing() {
    let ring = Ring::new();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.pushed(), 0);
    assert!(ring.in_order().is_empty());
}

#[test]
fn a_ring_under_its_size_keeps_every_record_in_order() {
    let mut ring = Ring::new();
    for i in 0..5 {
        ring.push(frame(i, 1000));
    }
    assert_eq!(ring.len(), 5);
    assert_eq!(ring.pushed(), 5);
    assert_eq!(ring.in_order().iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
}

#[test]
fn a_full_ring_keeps_the_newest_and_drops_the_oldest() {
    let mut ring = Ring::new();
    // Three wraps and a bit: the oldest kept is always `pushed - FRAMES`.
    let pushed = FRAMES as u64 * 3 + 7;
    for i in 0..pushed {
        ring.push(frame(i, 1000));
    }
    assert_eq!(ring.len(), FRAMES, "the ring never grows");
    assert_eq!(ring.pushed(), pushed, "how many the run recorded in all");
    let held = ring.in_order();
    assert_eq!(held.len(), FRAMES);
    assert_eq!(held.first().unwrap().index, pushed - FRAMES as u64, "oldest first");
    assert_eq!(held.last().unwrap().index, pushed - 1, "the newest is last");
    // Strictly one per frame, no gaps and no repeats.
    assert!(held.windows(2).all(|w| w[1].index == w[0].index + 1));
}

/// Exactly full, and one past full: the boundary the wrap arithmetic turns on.
#[test]
fn the_wrap_boundary_keeps_the_right_window() {
    let mut ring = Ring::new();
    for i in 0..FRAMES as u64 {
        ring.push(frame(i, 1));
    }
    assert_eq!(ring.in_order().first().unwrap().index, 0, "exactly full: nothing dropped yet");
    assert_eq!(ring.in_order().last().unwrap().index, FRAMES as u64 - 1);
    ring.push(frame(FRAMES as u64, 1));
    assert_eq!(ring.len(), FRAMES);
    assert_eq!(ring.in_order().first().unwrap().index, 1, "one past full: the first record is gone");
    assert_eq!(ring.in_order().last().unwrap().index, FRAMES as u64);
}

#[test]
fn feature_bits_read_as_their_names_in_bit_order() {
    assert_eq!(feature_text(0), "-");
    assert_eq!(feature_text(feature::AO), "ao");
    assert_eq!(feature_text(feature::WATER_REFLECTION | feature::LOADING), "load,water-reflection");
    assert_eq!(
        feature_text(feature::TERRAIN_SHADOWS_GPU | feature::WATER_PIXELS_READBACK | feature::MINIMAP),
        "terrain-shadows-gpu,water-pixels-readback,minimap"
    );
    // A bit no feature claims is ignored rather than printed as a number.
    assert_eq!(feature_text(1 << 31), "-");
}

const MB: u64 = 1024 * 1024;

fn sampled() -> Sampled {
    Sampled {
        age_secs: Some(3.25),
        allocated_bytes: Some(822 * MB),
        reserved_bytes: Some(1408 * MB),
        blocks: Some(14),
        allocations: Some(1203),
        vram_use_mb: Some((1420, 1520)),
        rss_bytes: Some(2390 * MB),
        wrist_cache_bytes: Some(3 * MB / 2),
    }
}

#[test]
fn the_sampled_figures_name_their_age_and_go_quiet_when_nothing_was_read() {
    let text = sampled().text();
    assert!(text.contains("gpu allocated 822 MB, reserved 1408 MB in 14 blocks, 1203 live allocations"), "{text}");
    assert!(text.contains("VRAM 1420 of 1520 MB"), "{text}");
    assert!(text.contains("read 3.2 s ago"), "{text}");
    // The main memory, where every other figure is the GPU's: a leak of ours shows here
    // (the out-of-memory abort of 2026-11-22), and the glance's caches with it.
    assert!(text.contains("resident 2390 MB"), "{text}");
    assert!(text.contains("wrist caches 1.5 MB"), "{text}");
    assert_eq!(Sampled::default().text(), "", "nothing read: no clause at all");
    // Only the budget read (no Vulkan allocator report): just that part.
    let budget_only = Sampled { vram_use_mb: Some((100, 200)), ..Sampled::default() };
    assert_eq!(budget_only.text(), "VRAM 100 of 200 MB");
    // A platform with no cheap reading, and a glance that has never been raised: no clause
    // of their own rather than a zero that reads as a figure.
    let no_rss = Sampled { rss_bytes: None, wrist_cache_bytes: Some(0), ..sampled() };
    assert!(!no_rss.text().contains("resident") && !no_rss.text().contains("wrist caches"), "{}", no_rss.text());
}

#[test]
fn the_summary_line_is_one_line_with_the_windows_totals() {
    let frames = vec![
        Frame { index: 10, frame_us: 16_000, uploads: 2, upload_bytes: MB, pipelines_busy: 3, features: feature::AO, ..Frame::EMPTY },
        Frame { index: 11, frame_us: 64_000, uploads: 1, upload_bytes: MB, pipelines_busy: 9, readbacks: 2, features: feature::LOADING, ..Frame::EMPTY },
    ];
    let line = report::summary_line(&frames, &sampled());
    assert!(!line.trim_end().contains('\n'), "the summary is one line: {line}");
    assert!(line.starts_with("gpu recorder: last 2 frames:"), "{line}");
    assert!(line.contains("40.0 ms mean"), "{line}");
    assert!(line.contains("64.0 ms worst"), "{line}");
    assert!(line.contains("uploads 3 writes, 2.0 MB"), "{line}");
    assert!(line.contains("pipelines busy at most 9"), "{line}");
    assert!(line.contains("readbacks at most 2"), "{line}");
    assert!(line.contains("ran load,ao"), "every feature of the window: {line}");
    assert_eq!(report::summary_line(&[], &Sampled::default()), "gpu recorder: no frames recorded yet");
}

#[test]
fn the_dump_has_a_row_a_frame_with_the_loss_the_sample_and_the_fault() {
    let frames = vec![
        Frame { index: 54_194, since_load_ms: Some(12_400), frame_us: 16_700, uploads: 3, upload_bytes: 2 * MB, pipelines_busy: 0, pipelines_asked: 812, readbacks: 1, features: feature::AO | feature::WATER_REFLECTION },
        Frame { index: 54_195, since_load_ms: Some(12_433), frame_us: 2_400_000, ..Frame::EMPTY },
    ];
    let fault = report::fault_text("GPU hang", &[("write to an invalid address".to_string(), 0xdead_beef, 0x1000)], &[("page fault".to_string(), 7, 9)]);
    let text = report::dump("Unknown Device is lost", &frames, 54_195 + 1, &sampled(), Some(&fault));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "gpu recorder: the graphics device was lost (Unknown Device is lost)");
    assert!(lines[1].contains("the last 2 of 54196 frames of this run, oldest first"), "{:?}", lines[1]);
    assert_eq!(lines[2], report::HEADER);
    // One row a frame, oldest first, with the frame's own figures.
    assert!(lines[3].contains("54194") && lines[3].contains("12.4") && lines[3].contains("16.7") && lines[3].contains("812"), "{:?}", lines[3]);
    assert!(lines[3].contains("water-reflection,ao"), "features in bit order: {:?}", lines[3]);
    assert!(lines[4].contains("54195") && lines[4].contains("2400.0"), "the 2.4 s frame: {:?}", lines[4]);
    assert!(lines[4].ends_with('-'), "no features that frame: {:?}", lines[4]);
    assert!(text.contains("gpu recorder: gpu allocated 822 MB"), "the sampled figures are in the dump");
    assert!(text.contains("VK_EXT_device_fault: GPU hang"), "the driver's fault info is in the dump");
    assert!(text.contains("VK_EXT_device_fault address: write to an invalid address at 0x00000000deadbeef (precision 0x1000)"), "{text}");
    assert!(text.contains("VK_EXT_device_fault vendor: page fault (code 0x7, data 0x9)"), "{text}");
}

#[test]
fn a_dump_without_frames_or_a_fault_still_says_what_happened() {
    let text = report::dump("no reason given", &[], 0, &Sampled::default(), None);
    assert!(text.contains("the graphics device was lost (no reason given)"));
    assert!(text.contains("no frames were recorded"));
    assert!(text.contains("VK_EXT_device_fault: nothing to report"), "a driver without the extension: said, not hidden");
    assert!(!text.contains("gpu allocated"));
}

#[test]
fn a_fault_without_infos_says_so_and_an_empty_description_is_named() {
    let text = report::fault_text("", &[], &[]);
    assert!(text.contains("VK_EXT_device_fault: (no description)"), "{text}");
    assert!(text.contains("no address or vendor infos"), "{text}");
}

/// The dump is bounded: the ring's size caps its rows whatever the run's length.
#[test]
fn the_dump_is_bounded_by_the_rings_size() {
    let mut ring = Ring::new();
    for i in 0..100_000u64 {
        ring.push(frame(i, 1000));
    }
    let frames = ring.in_order();
    let text = report::dump("lost", &frames, ring.pushed(), &Sampled::default(), None);
    // Heading, the frames line, the header, a row a frame, the fault line.
    assert_eq!(text.lines().count(), 3 + FRAMES + 1);
    assert!(text.len() < 64 * 1024, "the dump stays small enough for a log: {} bytes", text.len());
}

/// [`super::mark`] is additive within a frame and starts each frame clean.
#[test]
fn marks_add_up_and_are_taken_once() {
    use std::sync::atomic::Ordering::Relaxed;
    // The static is shared with the other tests in this binary, so this test only checks
    // the bits it sets itself, after taking whatever was there.
    let _ = super::FEATURES.swap(0, Relaxed);
    super::mark(feature::AO);
    super::mark(feature::MINIMAP);
    super::mark(feature::AO);
    let taken = super::FEATURES.swap(0, Relaxed);
    assert_eq!(taken & (feature::AO | feature::MINIMAP), feature::AO | feature::MINIMAP);
    assert_eq!(super::FEATURES.swap(0, Relaxed) & (feature::AO | feature::MINIMAP), 0, "taken once");
}
