use core::fmt::Debug;

// Keep the existing fatal path; the label identifies the failed map, not the cause
// of a preceding device loss. The successful path never formats a message.
#[track_caller]
pub(crate) fn expect_mapped(result: Result<(), impl Debug>, source: Option<&str>, bytes: u64) {
    if let Err(error) = result {
        let source = source.unwrap_or("unlabelled");
        panic!("Failed to map buffer: {error:?} (readback={source}, staging_bytes={bytes})");
    }
}

#[cfg(test)]
mod tests {
    use super::expect_mapped;

    // Mirrors the Debug representation of wgpu's map error without a GPU or wgpu.
    #[derive(Debug)]
    struct BufferAsyncError;

    #[test]
    #[should_panic(
        expected = "Failed to map buffer: BufferAsyncError (readback=water-pixels, staging_bytes=16)"
    )]
    fn failed_water_map_names_the_request() {
        expect_mapped(Err(BufferAsyncError), Some("water-pixels"), 16);
    }

    #[test]
    #[should_panic(
        expected = "Failed to map buffer: BufferAsyncError (readback=terrain-shadow-dump, staging_bytes=4096)"
    )]
    fn failed_terrain_dump_has_a_distinct_label() {
        expect_mapped(Err(BufferAsyncError), Some("terrain-shadow-dump"), 4096);
    }

    #[test]
    #[should_panic(expected = "Failed to map buffer: BufferAsyncError (readback=unlabelled, staging_bytes=8)")]
    fn failed_map_without_a_label_uses_the_fallback() {
        expect_mapped(Err(BufferAsyncError), None, 8);
    }

    #[test]
    fn successful_map_returns_normally() {
        expect_mapped(Ok::<_, BufferAsyncError>(()), Some("water-pixels"), 16);
    }
}
