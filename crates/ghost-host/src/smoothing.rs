#[derive(Debug, Clone, PartialEq)]
pub struct SmoothedValue {
    current: f64,
    target: f64,
    remaining_frames: usize,
}

impl SmoothedValue {
    pub fn new(value: f64) -> Self {
        Self {
            current: value,
            target: value,
            remaining_frames: 0,
        }
    }

    pub fn set_target(&mut self, target: f64, frames: usize) {
        self.target = target;
        self.remaining_frames = frames;
        if frames == 0 {
            self.current = target;
        }
    }

    pub fn advance(&mut self, frames: usize) -> (f64, f64) {
        let start = self.current;
        let advanced = frames.min(self.remaining_frames);
        if self.remaining_frames > 0 {
            let fraction = advanced as f64 / self.remaining_frames as f64;
            self.current += (self.target - self.current) * fraction;
            self.remaining_frames -= advanced;
        }
        if self.remaining_frames == 0 {
            self.current = self.target;
        }
        (start, self.current)
    }

    pub fn current(&self) -> f64 {
        self.current
    }
}
