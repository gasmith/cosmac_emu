use std::time::Duration;

#[derive(Debug)]
pub struct TimeTracker {
    real: Duration,
    emulated: Duration,
    speed: f64,
}
impl TimeTracker {
    pub fn new(speed: f64) -> Self {
        Self {
            real: Duration::default(),
            emulated: Duration::default(),
            speed,
        }
    }

    pub fn reset(&mut self) {
        self.real = Duration::default();
        self.emulated = Duration::default();
    }

    pub fn tick(&mut self, real: Duration, emulated: Duration) {
        self.real += real;
        self.emulated += emulated;
        let expect = Duration::from_secs_f64(self.emulated.as_secs_f64() / self.speed);
        if expect <= self.real {
            self.reset();
        } else {
            let delay = expect - self.real;
            if delay >= Duration::from_micros(10) {
                std::thread::sleep(delay);
                self.reset();
            }
        }
    }
}
