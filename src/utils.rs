pub fn humanize_seconds_to_minutes_and_seconds(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn get_current_executable_name() -> String {
    if let Some(executable_name) = try_get_current_executable_name() {
        return executable_name;
    }

    return "code-radio".to_string();
}

fn try_get_current_executable_name() -> Option<String> {
    std::env::current_exe()
        .ok()?
        .file_name()?
        .to_str()?
        .to_owned()
        .into()
}
