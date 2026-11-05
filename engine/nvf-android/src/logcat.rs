//! Sends the program's stdout and stderr to logcat (`adb logcat -s NVForever`).
//!
//! The game prints its start-up errors and warnings with `eprintln!` before its log system
//! exists (`error: ...`, `setup: ...`); Android discards both streams, so without this the
//! reason the game stopped is lost. Android only.

#[cfg(target_os = "android")]
pub fn forward_stdio() {
    use std::ffi::{c_char, c_int, CString};
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    const INFO: c_int = 4;

    // SAFETY: plain POSIX calls on descriptors this function creates; the read end is
    // handed to exactly one `File`, the write end replaces stdout and stderr.
    let read = unsafe {
        let mut fds = [0 as c_int; 2];
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return;
        }
        libc::dup2(fds[1], 1);
        libc::dup2(fds[1], 2);
        libc::close(fds[1]);
        std::fs::File::from_raw_fd(fds[0])
    };
    let _ = std::thread::Builder::new().name("nvf-stdio".into()).spawn(move || {
        let tag = CString::new("NVForever").expect("no NUL in the tag");
        for line in BufReader::new(read).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&line).replace('\0', " ");
            if let Ok(text) = CString::new(line) {
                // SAFETY: both pointers are valid NUL-terminated strings for the call.
                unsafe {
                    __android_log_write(INFO, tag.as_ptr(), text.as_ptr());
                }
            }
        }
    });
}

#[cfg(not(target_os = "android"))]
pub fn forward_stdio() {}
