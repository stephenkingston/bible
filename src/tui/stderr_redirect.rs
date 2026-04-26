//! Cross-platform stderr suppression for the duration of the TUI session.
//!
//! Some dependencies (notably `bibleref` 0.4) call `dbg!()` from within their
//! parser, which writes to stderr. While the TUI is in the alternate-screen
//! buffer, those writes go to the underlying terminal and visibly scroll the
//! TUI display, pushing the header off-screen.
//!
//! `silence()` redirects stderr to /dev/null (or NUL on Windows) and saves the
//! original. `restore()` puts the original back. Both are idempotent.

#[cfg(unix)]
mod imp {
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicI32, Ordering};

    static SAVED: AtomicI32 = AtomicI32::new(-1);

    pub fn silence() {
        if SAVED.load(Ordering::Relaxed) >= 0 {
            return;
        }
        let saved = unsafe { libc::dup(2) };
        if saved < 0 {
            return;
        }
        let null = match std::fs::OpenOptions::new().write(true).open("/dev/null") {
            Ok(f) => f,
            Err(_) => {
                unsafe { libc::close(saved) };
                return;
            }
        };
        unsafe { libc::dup2(null.as_raw_fd(), 2) };
        // dropping `null` closes the original /dev/null fd; fd 2 has its own
        // dup'd reference so writes to stderr keep going to /dev/null.
        SAVED.store(saved, Ordering::Relaxed);
    }

    pub fn restore() {
        let saved = SAVED.swap(-1, Ordering::Relaxed);
        if saved >= 0 {
            unsafe {
                libc::dup2(saved, 2);
                libc::close(saved);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::io::IntoRawHandle;
    use std::sync::atomic::{AtomicIsize, Ordering};

    const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4; // -12 cast to u32

    unsafe extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> isize;
        fn SetStdHandle(n_std_handle: u32, h_handle: isize) -> i32;
        fn CloseHandle(h_handle: isize) -> i32;
    }

    static SAVED: AtomicIsize = AtomicIsize::new(0);
    static NULL: AtomicIsize = AtomicIsize::new(0);

    pub fn silence() {
        if SAVED.load(Ordering::Relaxed) != 0 {
            return;
        }
        let saved = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        if saved == 0 || saved == -1 {
            return;
        }
        let null = match std::fs::OpenOptions::new().write(true).open("NUL") {
            Ok(f) => f,
            Err(_) => return,
        };
        let h = null.into_raw_handle() as isize;
        unsafe { SetStdHandle(STD_ERROR_HANDLE, h) };
        NULL.store(h, Ordering::Relaxed);
        SAVED.store(saved, Ordering::Relaxed);
    }

    pub fn restore() {
        let saved = SAVED.swap(0, Ordering::Relaxed);
        if saved != 0 {
            unsafe { SetStdHandle(STD_ERROR_HANDLE, saved) };
        }
        let null = NULL.swap(0, Ordering::Relaxed);
        if null != 0 {
            unsafe { CloseHandle(null) };
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    pub fn silence() {}
    pub fn restore() {}
}

pub use imp::{restore, silence};
