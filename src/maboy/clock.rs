pub struct Clock;

impl Clock {
    pub fn new() -> Clock {
        Clock
    }

    pub async fn cycle(&self) {
        unimplemented!()
    }

    pub async fn cycles(&self, count: u8) {
        for _ in 0..count {
            self.cycle().await;
        }
    }
}
