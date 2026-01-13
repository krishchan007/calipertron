#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use embassy_stm32::Config;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{Uart, Config as UartConfig};

use embedded_io::Write; // 👈 REQUIRED

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("uart_only (blocking) start");

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
    }

    let p = embassy_stm32::init(config);

    let mut uart_cfg = UartConfig::default();
    uart_cfg.baudrate = 115_200;

    let mut uart = Uart::new_blocking(
        p.USART1,
        p.PA10, // RX
        p.PA9,  // TX
        uart_cfg,
    )
    .unwrap();

    loop {
        uart.write_all(b"hello from uart\r\n").unwrap();
        Timer::after(Duration::from_secs(1)).await;
    }
}
