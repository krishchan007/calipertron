#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_stm32::{adc, Config};
use embassy_stm32::dma::{Transfer, TransferOptions};
use embassy_stm32::time::Hertz;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("adc+dma test start");

    // ================================
    // CLOCK CONFIGURATION (PLL @ 72 MHz)
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
    // ADC BASIC POWER-UP (Embassy)
    // ================================
    let _adc = adc::Adc::new(p.ADC1);

    // ================================
    // STM32F103 ADC MANUAL CONFIG
    // ================================
    let adc = embassy_stm32::pac::ADC1;

    // Enable DMA + continuous conversion
    adc.cr2().modify(|w| {
        w.set_dma(true);
        w.set_cont(true);
    });

    // One conversion in regular sequence
    adc.sqr1().modify(|w| w.set_l(0));

    // Select channel 0 (PA0)
    const CHANNEL: u8 = 0;
    adc.sqr3().modify(|w| w.set_sq(0, CHANNEL));

    // Sampling time (important on F1)
    adc.smpr2()
        .modify(|w| w.set_smp(CHANNEL as usize, adc::SampleTime::CYCLES41_5));

    // Power on ADC
    adc.cr2().modify(|w| w.set_adon(true));

    // --- CALIBRATION (MANDATORY ON F1) ---
    adc.cr2().modify(|w| w.set_rstcal(true));
    while adc.cr2().read().rstcal() {}

    adc.cr2().modify(|w| w.set_cal(true));
    while adc.cr2().read().cal() {}

    // ================================
    // DMA BUFFER
    // ================================
    static mut BUF: [u16; 32] = [0; 32];

    loop {
        let buf = unsafe { &mut BUF };

        // DMA channel for ADC1
        let dma = unsafe {
            embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH1)
        };

        let req = embassy_stm32::adc::RxDma::request(&dma);
        let opts = TransferOptions::default();

        let transfer = unsafe {
            Transfer::new_read(
                dma,
                req,
                embassy_stm32::pac::ADC1.dr().as_ptr() as *mut u16,
                buf,
                opts,
            )
        };

        // ================================
        // START CONVERSION (THIS WAS MISSING)
        // ================================
        adc.cr2().modify(|w| w.set_swstart(true));

        // Wait for DMA completion
        transfer.await;

        info!("adc dma done");

        Timer::after_millis(1000).await;
    }
}
