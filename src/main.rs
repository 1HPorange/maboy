mod maboy;

use futures::executor::block_on;

fn main() {
    println!("Hello, world!");

    let clock = maboy::clock::Clock::new();

    let mut cpu = maboy::cpu::CPU::new();

    block_on(cpu.run(&clock)).expect("Emulation Error");
}
