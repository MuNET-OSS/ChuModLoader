#[derive(Clone)]
pub struct Config {
    pub frame_lock: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            frame_lock: None,
        }
    }
}

pub fn load() -> Config {
    Config::default()
}
