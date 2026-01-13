#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("math test start");

    let _p = embassy_stm32::init(Default::default());

    let mut x: f32 = 0.1234;

    loop {
        x = x * 1.001 + 0.0001;
        x = x - 0.00005;

        Timer::after_millis(1000).await;
        info!("math alive");
    }
}
