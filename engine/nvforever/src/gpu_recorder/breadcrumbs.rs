//! GPU breadcrumbs: which render-graph node the GPU was executing when it stopped.
//!
//! The markers themselves are recorded in the vendored `bevy_render`
//! (`bevy::render::gpu_breadcrumbs`, `engine/vendor/README.md`), because the hook has to sit
//! inside `RenderContext`, which every render-graph node records through, and that crate
//! cannot depend on this one. This module is the game's half: the switch's words at startup,
//! and what the markers read as in the dump a lost device gets.
//!
//! **What the markers are.** Immediately before the command buffer a node records, and
//! immediately after it, the queue is given a one-command buffer whose `vkCmdFillBuffer`
//! writes that marker's sequence number into its slot of a host-visible, host-coherent
//! buffer. After the loss the mapping still reads, so a slot holding the number the fill was
//! told to write is a marker the GPU *reached*. The last begin the GPU reached whose end it
//! never reached is the node that hung.
//!
//! **The switch.** `NVF_GPU_BREADCRUMBS=1` ([`crate::odin::GPU_BREADCRUMBS_ENV`]); the Android
//! launcher's "Crash test: GPU breadcrumbs" sets it (`nvf-android`). Off, nothing is
//! allocated and no marker is recorded.
//!
//! **What this cannot say.** A marker is a transfer write with no barrier before or after
//! it, so the GPU may overlap it with the node's own commands: "the GPU reached this node's
//! begin" is not "every command of the node before it retired". The whole window is printed
//! for that reason, rather than a single answer.

use bevy::prelude::*;
use bevy::render::gpu_breadcrumbs::{self, Mark};

/// Whether this run was asked to record breadcrumbs.
pub fn requested() -> bool {
    std::env::var(crate::odin::GPU_BREADCRUMBS_ENV).is_ok_and(|v| v.trim() != "0" && !v.trim().is_empty())
}

/// Says in the run log whether this run records breadcrumbs. The buffer itself is made on the
/// first marker, and says so then (`bevy::render::gpu_breadcrumbs`).
pub fn report_start() {
    if requested() {
        // warn, so no log level a player set can hide what this run is doing to its frames.
        warn!(
            "gpu breadcrumbs: on ({}, crash test): every render-graph node writes a marker before and after its work, \
             so a lost device names the node the GPU hung in. Costs a little CPU time a frame and nothing else.",
            crate::odin::GPU_BREADCRUMBS_ENV
        );
    } else {
        info!("gpu breadcrumbs: off ({}=1 turns them on); a lost device is recorded without the node the GPU was in", crate::odin::GPU_BREADCRUMBS_ENV);
    }
}

/// The dump's breadcrumb section, or `""` when nothing was recorded (the switch is off, the
/// backend is not Vulkan, or no node ever opened an encoder).
///
/// Reads only host memory and plain atomics, so it is safe on the path that handles the loss.
pub fn section() -> String {
    match gpu_breadcrumbs::marks() {
        Some(marks) => text(&marks),
        None => String::new(),
    }
}

/// What one frame's markers came to.
#[derive(Debug, Default, PartialEq, Eq)]
struct Summary {
    frame: u64,
    /// Markers recorded for the frame, and how many of them the GPU reached.
    recorded: usize,
    reached: usize,
    /// The last node whose begin marker the GPU reached, and the last whose end it reached.
    last_began: Option<String>,
    last_ended: Option<String>,
    /// Nodes the GPU began and never ended, oldest first: the suspects.
    open: Vec<String>,
}

/// The frames in the window, oldest first, each read as a [`Summary`].
fn summaries(marks: &[Mark]) -> Vec<Summary> {
    let mut out: Vec<Summary> = Vec::new();
    for mark in marks {
        if out.last().is_none_or(|last| last.frame != mark.frame) {
            out.push(Summary { frame: mark.frame, ..Summary::default() });
        }
        // `push` above makes sure of it.
        let Some(frame) = out.last_mut() else { continue };
        frame.recorded += 1;
        if !mark.ran {
            continue;
        }
        frame.reached += 1;
        if mark.end {
            frame.last_ended = Some(mark.node.clone());
            if let Some(at) = frame.open.iter().position(|open| *open == mark.node) {
                frame.open.remove(at);
            }
        } else {
            frame.last_began = Some(mark.node.clone());
            frame.open.push(mark.node.clone());
        }
    }
    out
}

/// A node name or a dash when there is none.
fn name(node: Option<&String>) -> &str {
    node.map_or("-", String::as_str)
}

/// The whole breadcrumb section: the window's totals, where the GPU stopped, and a line a
/// frame. Pure formatting over the markers, so it is tested without a GPU.
pub fn text(marks: &[Mark]) -> String {
    if marks.is_empty() {
        return "gpu breadcrumbs: on, but no marker was ever recorded\n".to_string();
    }
    let reached = marks.iter().filter(|m| m.ran).count();
    let mut out = String::with_capacity(128 + marks.len() * 8);
    out.push_str(&format!("gpu breadcrumbs: {} markers in the window, {reached} of them reached by the GPU\n", marks.len()));
    if reached == 0 {
        out.push_str("gpu breadcrumbs: the GPU reached none of them: it stopped before the oldest marker the window still holds\n");
        return out;
    }
    // Where the GPU stopped: the last marker it reached, and the first it did not.
    if let Some(last) = marks.iter().rfind(|m| m.ran) {
        let what = if last.end { "ended" } else { "began" };
        out.push_str(&format!("gpu breadcrumbs: the last marker the GPU reached: frame {} {what} {}\n", last.frame, last.node));
        if let Some(next) = marks.iter().find(|m| m.seq > last.seq) {
            let what = if next.end { "end of" } else { "start of" };
            out.push_str(&format!("gpu breadcrumbs: the next marker it never reached: frame {} {what} {}\n", next.frame, next.node));
        }
    }
    for frame in summaries(marks) {
        let open = if frame.open.is_empty() { "none".to_string() } else { frame.open.join(", ") };
        out.push_str(&format!(
            "gpu breadcrumbs: frame {}: {} markers, {} reached; last began {}, last ended {}; began and never ended: {open}\n",
            frame.frame,
            frame.recorded,
            frame.reached,
            name(frame.last_began.as_ref()),
            name(frame.last_ended.as_ref()),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A marker, as the vendored recorder would have left it.
    fn mark(seq: u32, frame: u64, node: &str, end: bool, ran: bool) -> Mark {
        Mark { seq, frame, node: node.to_string(), end, ran }
    }

    /// One frame the GPU finished: every marker reached, nothing left open.
    #[test]
    fn a_breadcrumb_frame_the_gpu_finished_leaves_nothing_open() {
        let marks = vec![
            mark(1, 7, "prepass", false, true),
            mark(2, 7, "prepass", true, true),
            mark(3, 7, "opaque", false, true),
            mark(4, 7, "opaque", true, true),
        ];
        let text = text(&marks);
        assert!(text.contains("4 markers in the window, 4 of them reached"), "{text}");
        assert!(text.contains("frame 7: 4 markers, 4 reached; last began opaque, last ended opaque; began and never ended: none"), "{text}");
        // Nothing was left unreached, so there is no "never reached" line.
        assert!(!text.contains("never reached"), "{text}");
    }

    /// The hang: a node the GPU began and never ended, and the marker it never reached.
    #[test]
    fn a_breadcrumb_hang_names_the_node_that_began_and_never_ended() {
        let marks = vec![
            mark(10, 42, "prepass", false, true),
            mark(11, 42, "prepass", true, true),
            mark(12, 42, "terrain", false, true),
            mark(13, 42, "terrain", true, false),
            mark(14, 42, "ui", false, false),
            mark(15, 42, "ui", true, false),
        ];
        let text = text(&marks);
        assert!(text.contains("6 markers in the window, 3 of them reached"), "{text}");
        assert!(text.contains("the last marker the GPU reached: frame 42 began terrain"), "{text}");
        assert!(text.contains("the next marker it never reached: frame 42 end of terrain"), "{text}");
        assert!(text.contains("last began terrain, last ended prepass; began and never ended: terrain"), "{text}");
    }

    /// Several frames each get their own line, oldest first, and a node that ran twice in a
    /// frame is closed once per end.
    #[test]
    fn breadcrumb_frames_are_summarised_one_line_each() {
        let marks = vec![
            mark(1, 3, "shadows", false, true),
            mark(2, 3, "shadows", true, true),
            mark(3, 4, "shadows", false, true),
            mark(4, 4, "shadows", false, true),
            mark(5, 4, "shadows", true, true),
        ];
        let whole = text(&marks);
        let lines: Vec<&str> = whole.lines().filter(|l| l.contains("frame 3:") || l.contains("frame 4:")).collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("frame 3: 2 markers, 2 reached"), "{lines:?}");
        // Two begins, one end: one of the two is still open.
        assert!(lines[1].contains("frame 4: 3 markers, 3 reached"), "{lines:?}");
        assert!(lines[1].ends_with("began and never ended: shadows"), "{lines:?}");
    }

    /// Nothing recorded, and nothing reached, both read as sentences rather than an empty
    /// section.
    #[test]
    fn breadcrumbs_say_so_when_there_is_nothing_to_report() {
        assert_eq!(text(&[]), "gpu breadcrumbs: on, but no marker was ever recorded\n");
        let none = text(&[mark(1, 1, "opaque", false, false)]);
        assert!(none.contains("1 markers in the window, 0 of them reached"), "{none}");
        assert!(none.contains("it stopped before the oldest marker the window still holds"), "{none}");
        assert!(!none.contains("frame 1:"), "{none}");
    }
}
