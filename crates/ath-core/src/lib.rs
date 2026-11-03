//! Shared record library used by the command-line tool and by tests.

pub mod apple;
pub mod calc;
pub mod fit;
pub mod garmin;
pub mod mapping;
pub mod oura;
pub mod persist;
pub mod range_calc;
pub mod personal;
pub mod store;
pub mod strava;
pub mod time;
pub mod whoop;

/// The program version. Revisions this library writes name it as the tool version.
pub const APP_VERSION: &str = "2.1.0";

/// The record version a database must store before this library opens it.
pub const RECORD_VERSION: &str = "2.1.0";
