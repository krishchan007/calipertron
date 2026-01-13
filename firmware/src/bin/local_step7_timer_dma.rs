#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_stm32::Config;
use embassy_stm32::dma::{Transfer, TransferOptions};
use embassy_stm32::time::Hertz;

// ================================
// DMA BUFFERS (MUST BE AT MODULE SCOPE)
// ================================
static mut BUF: [u32; 1] = [0];
static SRC: [u32; 1] = [0xDEADBEEF];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("timer-dma test start");

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
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }

    let p = embassy_stm32::init(config);

    // ================================
    // TIMER SETUP (TIM2 @ 1 Hz)
    // ================================
    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM2);
    let regs = tim.regs_gp16();

    // Enable update DMA request
    regs.dier().modify(|w| w.set_ude(true));

    // 1 Hz update rate
    tim.set_frequency(Hertz(1));

    loop {
        let buf = unsafe { &mut BUF };

        let dma = unsafe {
            embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH2)
        };

        let req = embassy_stm32::timer::UpDma::request(&dma);
        let opts = TransferOptions::default();

        let transfer = unsafe {
            Transfer::new_write(
                dma,
                req,
                &SRC,
                buf.as_mut_ptr(),
                opts,
            )
        };

        tim.reset();
        tim.start();

        transfer.await;

        info!("timer dma done");

        Timer::after_millis(1000).await;
    }
}
