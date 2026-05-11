use chrono::{DateTime, Local, TimeZone};
use humantime::format_duration;
use std::time::Duration;

pub fn seconds_to_human_readable(seconds: u64) -> String {
    let duration = Duration::new(seconds, 0);
    format_duration(duration).to_string()
}

pub fn get_timestamp_local() -> String {
    let dt1: DateTime<Local> = Local::now();
    let formatted = dt1.format("%B %d %Y %r");
    format!("{}", formatted)
}
