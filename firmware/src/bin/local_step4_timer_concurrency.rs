#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("multi-task start");

    // REQUIRED for embassy_time
    let _p = embassy_stm32::init(Default::default());

    spawner.spawn(task1()).unwrap();
    spawner.spawn(task2()).unwrap();
}


#[embassy_executor::task]
async fn task1() {
    loop {
        info!("task1");
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::task]
async fn task2() {
    loop {
        Timer::after_millis(700).await;
    }
}