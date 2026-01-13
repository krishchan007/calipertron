#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("logging test start");

    // REQUIRED for embassy_time
    let _p = embassy_stm32::init(Default::default());

    loop {
        info!("fast log");
        Timer::after_millis(10).await; // 100 Hz
    }
}
