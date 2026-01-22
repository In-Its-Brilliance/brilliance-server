use std::time::Instant;

pub struct RuntimeSpan {
    name: &'static str,
    start: Instant,
}

impl RuntimeSpan {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for RuntimeSpan {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        crate::debug::STORAGE.lock().unwrap().push(self.name, elapsed);
    }
}
