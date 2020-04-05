use std::future::Future;

const CYCLES_PER_SEC: u32 = 4_194_304;
pub const MCYCLES_PER_SEC: u32 = CYCLES_PER_SEC / 4;

/// For now, this yields once every 4 ticks (machine cycle)
pub async fn ticks(n: u8) {
    for _ in 0..n / 4 {
        futures::pending!()
    }
}

pub struct DummyWaker;

impl futures::task::ArcWake for DummyWaker {
    fn wake_by_ref(arc_self: &std::sync::Arc<Self>) {}
}
