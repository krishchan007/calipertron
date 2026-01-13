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

// DMA source: toggle PA0
static SRC: [u32; 1] = [1 << 0]; // GPIOA_BSRR set PA0
static mut DUMMY: [u32; 1] = [0];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("gpio-dma test start");

    // --- PLL clocks ---
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

    // Configure PA0 as output
    let _pa0 = Output::new(p.PA0, Level::Low, Speed::Low);

    // TIM2 @ 1 Hz
    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM2);
    let regs = tim.regs_gp16();
    regs.dier().modify(|w| w.set_ude(true));
    tim.set_frequency(Hertz(1));

    loop {
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
                embassy_stm32::pac::GPIOA.bsrr().as_ptr() as *mut u32,
                opts,
            )
        };

        tim.reset();
        tim.start();

        transfer.await;

        info!("gpio dma done");

        Timer::after_millis(1000).await;
    }
}
