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
    ppu::PPU,
    windows::gfx::{GfxDevice, GfxWindow},
    windows::window::Window,
};
use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let cartridge =
        // maboy::cartridge::Cartridge::from_rom("./roms/Tetris (World) (Rev A).gb").unwrap();
        maboy::cartridge::Cartridge::from_rom("./roms/test.gb").unwrap();

    // We need these two dummies since we want to use async without a runtime
    let dummy_waker = waker(std::sync::Arc::new(DummyWaker));
    let mut dummy_async_context = Context::from_waker(&dummy_waker);

    let mut cpu = CPU::new();
    let mut ppu = PPU::new();

    let mut builtin_mem = BuiltinMem::new();
    //let mut cartridge_mem = CartridgeMem::empty();
    let mut cartridge_mem = cartridge.mem;

    let mmu = UnsafeCell::new(MMU::TEMP_NEW(&mut builtin_mem, &mut cartridge_mem));

    let window = Window::new().unwrap();

    let gfx_device = GfxDevice::new().expect("Failed to init gfx");
    let mut gfx_window = gfx_device
        .attach_to_window(&window)
        .expect("Failed create swapchain for window");

    // TODO: remove first frame stuff, or think about it
    let mut first_frame = gfx_window.next_frame();
    first_frame.clear(&[0.5, 0.5, 0.5, 1.0]);
    first_frame.present(false).expect("Lost graphics device");

    window.show();

    let mut cpu_task = Box::pin(cpu.step(unsafe { &mut *mmu.get() }));
    let mut ppu_task = Box::pin(ppu.step(&mmu, gfx_window));

    while true
    // window.handle_msgs() TODO: Avoid calling this so often, or at least think about it
    {
        cpu_task.as_mut().poll(&mut dummy_async_context);
        ppu_task.as_mut().poll(&mut dummy_async_context);
    }
}
