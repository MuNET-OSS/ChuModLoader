#[derive(Clone, Default)]
pub struct Config {
    pub frame_lock: Option<u32>,
}

pub fn load() -> Config {
    Config::default()
}
