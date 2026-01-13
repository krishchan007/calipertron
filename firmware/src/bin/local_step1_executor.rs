#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("executor-only start");

    // REQUIRED:
    // This installs the Embassy time driver.
    let _p = embassy_stm32::init(Default::default());

    loop {
        Timer::after_millis(1000).await;
        info!("alive");
    }
}
