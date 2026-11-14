//! Test-only failure injection at file and SQL boundaries.
//!
//! This module exists only in unit-test builds. Release builds contain no
//! failure points. A test arms a point on its own thread to make the operation
//! fail there and run its normal error path. A test can also run itself in a
//! child process with `ATH_TEST_ABORT_AT` set; the child then stops abruptly at
//! that point, as a killed process would, and the parent checks what remains.

use std::cell::RefCell;

use crate::error::{Error, Result};

thread_local! {
    static ARMED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Make the named point fail on this thread until `disarm_all`.
pub(crate) fn arm(name: &str) {
    ARMED.with(|armed| armed.borrow_mut().push(name.to_string()));
}

pub(crate) fn disarm_all() {
    ARMED.with(|armed| armed.borrow_mut().clear());
}

/// The environment variable that stops a child test process at a point.
pub(crate) const ABORT_VARIABLE: &str = "ATH_TEST_ABORT_AT";

/// Fail or stop at `name` when a test asked for it.
pub(crate) fn check(name: &str) -> Result<()> {
    if std::env::var(ABORT_VARIABLE).as_deref() == Ok(name) {
        std::process::abort();
    }
    let armed = ARMED.with(|armed| armed.borrow().iter().any(|point| point == name));
    if armed {
        return Err(Error::other(format!("injected failure at {name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_armed_point_fails_only_on_its_thread() {
        super::arm("example.point");
        assert!(super::check("example.point").is_err());
        assert!(super::check("other.point").is_ok());
        assert!(std::thread::spawn(|| super::check("example.point").is_ok()).join().unwrap());
        super::disarm_all();
        assert!(super::check("example.point").is_ok());
    }
}
