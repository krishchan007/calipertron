#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_stm32::gpio::{Output, Level, Speed};
use embassy_stm32::dma::{Transfer, TransferOptions};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

// Circular DMA source pattern: toggle PA0
static SRC: [u32; 2] = [
    1 << 0,          // set PA0
    1 << (0 + 16),   // reset PA0
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("circular gpio dma start");

    // ================================
    // CLOCKS: PLL @ 72 MHz
    // ================================
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }

    let p = embassy_stm32::init(config);

    // Configure PA0
    let _pa0 = Output::new(p.PA0, Level::Low, Speed::Low);

    // TIM2 @ high frequency (INTENTIONAL)
    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM2);
    let regs = tim.regs_gp16();
    regs.dier().modify(|w| w.set_ude(true));
    tim.set_frequency(Hertz(20_000));

    // DMA channel
    let dma = unsafe {
        embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH2)
    };

    let req = embassy_stm32::timer::UpDma::request(&dma);

    let mut opts = TransferOptions::default();
    opts.circular = true;

    // Start circular DMA to GPIOA_BSRR
    let _transfer = unsafe {
        Transfer::new_write(
            dma,
            req,
            &SRC,
            embassy_stm32::pac::GPIOA.bsrr().as_ptr() as *mut u32,
            opts,
        )
    };

    tim.start();

    // ================================
    // 🔴 DEBUG-STARVATION SECTION
    // ================================
    loop {
        // Tight CPU-bound loop (NO await)
        for _ in 0..100_000 {
            core::hint::black_box(12345_u32.wrapping_mul(1664525));
        }

        // RTT logging inside starved context
        info!("still alive");

        // Small delay so logs appear briefly
        Timer::after_millis(100).await;
    }
}