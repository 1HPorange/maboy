mod maboy;
// mod maboy_old;

fn main() {
    let cpu = maboy::cpu::CPU::new();
    cpu.step_instr(fuck);
}
