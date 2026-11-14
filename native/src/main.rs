//! The `ath` executable: run the command line and exit with its status.
//!
//! `ath __client` alone starts one JavaScript client protocol worker instead
//! (see `athleticstandard::protocol`). The command line never calls it.

fn main() {
    let words: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if athleticstandard::protocol::selects_worker(&words) {
        std::process::exit(athleticstandard::protocol::serve());
    }
    std::process::exit(athleticstandard::cli::main());
}
