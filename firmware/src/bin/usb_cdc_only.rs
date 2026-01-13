#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use embassy_stm32::Config;
use embassy_stm32::time::Hertz;
use embassy_stm32::usb::Driver;

use embassy_usb::{Builder, Config as UsbConfig};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcState};

use static_cell::StaticCell;

// ================================
// USB INTERRUPT BINDING
// ================================
embassy_stm32::bind_interrupts!(struct UsbIrqs {
    USB_LP_CAN1_RX0 => embassy_stm32::usb::InterruptHandler<
        embassy_stm32::peripherals::USB
    >;
});

// ================================
// STATIC STORAGE
// ================================
static USB_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CFG:  StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS:  StaticCell<[u8; 256]> = StaticCell::new();
static USB_MSOS: StaticCell<[u8; 256]> = StaticCell::new();
static CDC_STATE: StaticCell<CdcState<'static>> = StaticCell::new();

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<
        'static,
        Driver<'static, embassy_stm32::peripherals::USB>,
    >,
) {
    usb.run().await;
}

#[embassy_executor::task]
async fn cdc_task(
    mut cdc: CdcAcmClass<
        'static,
        Driver<'static, embassy_stm32::peripherals::USB>,
    >,
) {
    loop {
        let _ = cdc.write_packet(b"hello over usb\r\n").await;
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("USB CDC start");

    // ================================
    // CLOCK CONFIG (72 MHz)
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
        config.rcc.ahb_pre  = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }

    let p = embassy_stm32::init(config);

    // ================================
    // USB DRIVER
    // ================================
    let driver = Driver::new(
        p.USB,
        UsbIrqs,
        p.PA12, // D+
        p.PA11, // D-
    );

    // ================================
    // USB CONFIG — STANDARD CDC
    // ================================
    let mut usb_cfg = UsbConfig::new(0xCafe, 0x4011);
    usb_cfg.manufacturer = Some("Calipertron");
    usb_cfg.product = Some("USB CDC Device");
    usb_cfg.serial_number = Some("0001");

    // 🔑 THIS IS THE IMPORTANT PART
    usb_cfg.device_class = 0x02;        // CDC
    usb_cfg.device_sub_class = 0x02;    // ACM
    usb_cfg.device_protocol = 0x01;     // AT commands

    let mut builder = Builder::new(
        driver,
        usb_cfg,
        USB_DESC.init([0; 256]),
        USB_CFG.init([0; 256]),
        USB_BOS.init([0; 256]),
        USB_MSOS.init([0; 256]),
    );

    let cdc = CdcAcmClass::new(
        &mut builder,
        CDC_STATE.init(CdcState::new()),
        64,
    );

    let usb = builder.build();

    spawner.spawn(usb_task(usb)).unwrap();
    spawner.spawn(cdc_task(cdc)).unwrap();

    loop {
        Timer::after(Duration::from_secs(10)).await;
    }
}
