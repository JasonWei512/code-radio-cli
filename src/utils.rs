pub fn humanize_seconds_to_minutes_and_seconds(seconds: i64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
