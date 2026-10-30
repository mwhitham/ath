//! Shared record library used by the command-line tool and by tests.

pub mod apple;
pub mod calc;
pub mod fit;
pub mod garmin;
pub mod oura;
pub mod persist;
pub mod range_calc;
pub mod personal;
pub mod store;
pub mod strava;
pub mod time;
pub mod whoop;

pub const SOFTWARE_VERSION: &str = "2.0.0";
