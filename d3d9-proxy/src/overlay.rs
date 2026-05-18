pub struct FpsState {
    last_frame_time: u64,
    freq: u64,
}

impl FpsState {
    pub fn new() -> Self {
        let mut freq = 0i64;
        let mut now = 0i64;
        unsafe {
            QueryPerformanceFrequency(&mut freq);
            QueryPerformanceCounter(&mut now);
        }
        Self {
            last_frame_time: now as u64,
            freq: freq as u64,
        }
    }

    pub fn frame_lock(&mut self, target_fps: u32) {
        if target_fps == 0 || self.freq == 0 {
            return;
        }
        let target_interval = self.freq / target_fps as u64;
        loop {
            let mut now = 0i64;
            unsafe { QueryPerformanceCounter(&mut now); }
            if now as u64 - self.last_frame_time >= target_interval {
                self.last_frame_time = now as u64;
                return;
            }
            std::hint::spin_loop();
        }
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn QueryPerformanceCounter(count: *mut i64) -> i32;
    fn QueryPerformanceFrequency(freq: *mut i64) -> i32;
}
