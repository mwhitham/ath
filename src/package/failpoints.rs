//! Test-only failure injection at file and SQL boundaries.
//!
//! This module exists only in unit-test builds. Release builds contain no
//! failure points. A test arms a point on its own thread to make the operation
//! fail there and run its normal error path. A test can also run itself in a
//! child process with `ATH_TEST_STOP_AT` set; the child then exits at once at
//! that point, with no cleanup, as a killed process would, and the parent
//! checks what remains. A hook runs other work at a point, such as a second
//! writer during an export.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::{Error, Result};

type Hook = Box<dyn FnMut()>;

thread_local! {
    static ARMED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static HOOKS: RefCell<HashMap<String, Hook>> = RefCell::new(HashMap::new());
}

/// Make the named point fail on this thread until `disarm_all`.
pub(crate) fn arm(name: &str) {
    ARMED.with(|armed| armed.borrow_mut().push(name.to_string()));
}

/// Run `hook` each time this thread reaches the named point.
pub(crate) fn on(name: &str, hook: impl FnMut() + 'static) {
    HOOKS.with(|hooks| hooks.borrow_mut().insert(name.to_string(), Box::new(hook)));
}

pub(crate) fn disarm_all() {
    ARMED.with(|armed| armed.borrow_mut().clear());
    HOOKS.with(|hooks| hooks.borrow_mut().clear());
}

/// The environment variable that stops a child test process at a point.
pub(crate) const STOP_VARIABLE: &str = "ATH_TEST_STOP_AT";

/// The exit status of a child stopped at a point.
pub(crate) const STOP_STATUS: i32 = 86;

/// Fail, stop, or run a hook at `name` when a test asked for it.
pub(crate) fn check(name: &str) -> Result<()> {
    if std::env::var(STOP_VARIABLE).as_deref() == Ok(name) {
        // Exit without unwinding: no destructor, rollback, or cleanup runs.
        std::process::exit(STOP_STATUS);
    }
    let mut hook = HOOKS.with(|hooks| hooks.borrow_mut().remove(name));
    if let Some(hook) = hook.as_mut() {
        hook();
    }
    if let Some(hook) = hook {
        HOOKS.with(|hooks| hooks.borrow_mut().insert(name.to_string(), hook));
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

    #[test]
    fn a_hook_runs_at_its_point() {
        let count = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = count.clone();
        super::on("hook.point", move || seen.set(seen.get() + 1));
        super::check("hook.point").unwrap();
        super::check("hook.point").unwrap();
        assert_eq!(count.get(), 2);
        super::disarm_all();
        super::check("hook.point").unwrap();
        assert_eq!(count.get(), 2);
    }
}
