//! Whether another process still owns work on disk.
//!
//! Cleanup removes staged files only when their owner has stopped. A staging
//! marker or a pending file operation names its owner by process ID. A process
//! ID that the system reuses for an unrelated program keeps the work in place;
//! that delays cleanup but never removes active work.

/// This process's ID.
pub fn current_pid() -> u32 {
    std::process::id()
}

/// True when a process with this ID is running. Zero and IDs the system cannot
/// represent are never running.
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == current_pid() {
        return true;
    }
    platform::is_alive(pid)
}

#[cfg(unix)]
mod platform {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    /// `ESRCH` is 3 on Linux, macOS, and the BSDs.
    const NO_SUCH_PROCESS: i32 = 3;

    pub fn is_alive(pid: u32) -> bool {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        // SAFETY: signal 0 only checks that the process exists and that this
        // process may signal it. It sends nothing and touches no memory.
        let result = unsafe { kill(pid, 0) };
        if result == 0 {
            return true;
        }
        // EPERM means the process exists but belongs to another user.
        std::io::Error::last_os_error().raw_os_error() != Some(NO_SUCH_PROCESS)
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: i32 = 5;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    pub fn is_alive(pid: u32) -> bool {
        // SAFETY: the handle is checked before use and closed exactly once.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // A process that exists but denies access is still running.
                return std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            ok != 0 && code == STILL_ACTIVE
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    /// Without a way to ask, assume the owner is running and keep its work.
    pub fn is_alive(_pid: u32) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_alive_and_an_unused_id_is_not() {
        assert!(is_process_alive(current_pid()));
        assert!(!is_process_alive(0));
        assert!(!is_process_alive(99_999_999));
    }
}
