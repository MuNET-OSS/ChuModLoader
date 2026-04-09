fn main() {
    let system_drive = std::env::var("SYSTEMDRIVE").unwrap_or("C:".to_string());
    let version_path = format!("{}\\Windows\\System32\\version.dll", system_drive);
    forward_dll::forward_dll(&version_path).unwrap();
}
