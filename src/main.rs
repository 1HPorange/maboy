mod maboy;
use futures::executor::block_on;
use futures::future::FutureExt;
use futures::task::waker;
use maboy::{
    cartridge::Cartridge,
    clock,
    clock::DummyWaker,
    cpu::CPU,
    mmu::{BuiltinMem, CartridgeMem, MMU},
    windows::gfx::{GfxDevice, GfxWindow},
    windows::window::Window,
};
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    // let cartridge =
    //     maboy::cartridge::Cartridge::from_rom("./roms/Tetris (World) (Rev A).gb").unwrap();

    // We need these two dummies since we want to use async without a runtime
    let dummy_waker = waker(std::sync::Arc::new(DummyWaker));
    let mut dummy_async_context = Context::from_waker(&dummy_waker);

    let mut cpu = CPU::new();

    let mut builtin_mem = BuiltinMem::new();
    let mut cartridge_mem = CartridgeMem::empty();
    let mut mmu = MMU::TEMP_NEW(&mut builtin_mem, &mut cartridge_mem);

    let window = Window::new().unwrap();

    let gfx_device = GfxDevice::new().expect("Failed to init gfx");
    let mut gfx_window = gfx_device
        .attach_to_window(&window)
        .expect("Failed create swapchain for window");

    window.show();

    let mut cpu_task = Box::pin(cpu.run(&mut mmu));

    while window.handle_msgs() {
        let mut frame = gfx_window.next_frame(); // TODO: Only render frames when i should
        frame.clear(&[0.109, 0.250, 0.156, 1.0]);

        cpu_task.as_mut().poll(&mut dummy_async_context);

        frame.present().expect("Graphics output lost");
        // TODO: Replace with multimedia timer API
        sleep(Duration::from_nanos(
            (1_000_000_000 / clock::MCYCLES_PER_SEC) as u64,
        ));
    }

    // block_on(cpu.run(&clock, &mut mmu));
}
