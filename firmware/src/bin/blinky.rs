#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("blinky start");

    // IMPORTANT:
    // Use Embassy's default config.
    // Do NOT touch RCC for now.
    let p = embassy_stm32::init(Default::default());

    // STM32F103C8 onboard LED is typically PC13
    let mut led = Output::new(p.PC13, Level::High, Speed::Low);

    loop {
        led.toggle();
        info!("tick");
        Timer::after_millis(1000).await;
    }
}
