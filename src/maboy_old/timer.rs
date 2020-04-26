use super::clock;
use super::cpu::Interrupt;
use super::mmu::MMU;
use super::util::Bit;

// TODO: Think about moving registers into async fn... Or not??? Harder to debug...
pub async fn step(mmu: &mut MMU<'_>) {
    // 4194304
    const INCR_DIV_FREQ: u8 = 16; // 256 / 16
    let mut incr_div_countdown = INCR_DIV_FREQ;

    // TODO: Explain this magic
    let mut incr_tima_countdown = 64u8;

    loop {
        clock::ticks(16).await;

        incr_div_countdown -= 1;
        if incr_div_countdown == 0 {
            // TODO: Implement reset through write on this register
            let div = mmu.mut8(0xFF04).unwrap();
            *div = div.wrapping_add(1);
            incr_div_countdown = INCR_DIV_FREQ;
        }

        incr_tima_countdown = incr_tima_countdown.saturating_sub(get_tac_freq_countdown(mmu));
        if incr_tima_countdown == 0 {
            let tma = mmu.read8(0xFF06);
            let tima = mmu.mut8(0xFF05).unwrap();
            *tima = tima.checked_add(1).unwrap_or(tma);
            mmu.request_interrupt(Interrupt::Timer);
            incr_tima_countdown = 64;
        }
    }
}

fn get_tac_freq_countdown(mmu: &mut MMU) -> u8 {
    let tac = mmu.read8(0xFF07);

    // TODO: Model bit 2 operation correctly
    if !tac.bit(2) {
        return 0;
    }

    match tac & 0b11 {
        0b00 => 1,
        0b01 => 64,
        0b10 => 16,
        0b11 => 4,
        _ => unsafe { std::hint::unreachable_unchecked() },
    }
}
