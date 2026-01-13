#![no_std]
#![no_main]

use calipertron_core::*;

#[cfg(feature = "debug")]
use defmt::*;
#[cfg(feature = "debug")]
use defmt_rtt as _;
#[cfg(feature = "debug")]
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use embassy_stm32::dma::*;
use embassy_stm32::gpio::{Flex, Input, Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{adc, Config};
use embassy_stm32::usb::Driver;

use embassy_usb::{Builder, Config as UsbConfig};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcState};

use heapless::String;
use heapless::spsc::Queue;
use core::cell::RefCell;

#[cfg(feature = "debug")]
use embassy_sync::mutex::Mutex;
#[cfg(feature = "debug")]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use static_cell::StaticCell;
use num_traits::Float;


// ================================
// USB INTERRUPTS (STM32F103)
// ================================
embassy_stm32::bind_interrupts!(struct UsbIrqs {
    USB_LP_CAN1_RX0 => embassy_stm32::usb::InterruptHandler<
        embassy_stm32::peripherals::USB
    >;
});

// ================================
// USB + LOGGING STATICS
// ================================
static USB_DESC: StaticCell<[u8; 128]> = StaticCell::new();
static USB_CFG:  StaticCell<[u8; 128]> = StaticCell::new();
static USB_BOS:  StaticCell<[u8; 64]> = StaticCell::new();
static USB_MSOS: StaticCell<[u8; 64]> = StaticCell::new();
static CDC_STATE: StaticCell<CdcState<'static>> = StaticCell::new();

static USB_QUEUE: Mutex<
    CriticalSectionRawMutex,
    RefCell<Queue<String<128>, 16>>,
> = Mutex::new(RefCell::new(Queue::new()));

include!(concat!(env!("OUT_DIR"), "/constants.rs"));
const NUM_SAMPLES: usize = SINE_COSINE_TABLE.len();

// ================================
// USB TASKS
// ================================
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
    cdc.wait_connection().await;

    loop {
        if let Some(msg) = USB_QUEUE
            .lock()
            .await
            .borrow_mut()
            .dequeue()
        {
            let _ = cdc.write_packet(msg.as_bytes()).await;
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}

// ================================
// MAIN
// ================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
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

    info!("local_inner_loop_usb start");

    // ================================
    // USB SETUP
    // ================================
    let driver = Driver::new(
        p.USB,
        UsbIrqs,
        p.PA12, // D+
        p.PA11, // D-
    );

    let mut usb_cfg = UsbConfig::new(0xCafe, 0x4011);
    usb_cfg.manufacturer = Some("Calipertron");
    usb_cfg.product = Some("Calipertron CDC");
    usb_cfg.serial_number = Some("0001");
    usb_cfg.device_class = 0x02;
    usb_cfg.device_sub_class = 0x02;
    usb_cfg.device_protocol = 0x01;

    let mut builder = Builder::new(
    driver,
    usb_cfg,
    USB_DESC.init([0; 128]),
    USB_CFG.init([0; 128]),
    USB_BOS.init([0; 64]),
    USB_MSOS.init([0; 64]),
);


    let cdc = CdcAcmClass::new(
        &mut builder,
        CDC_STATE.init(CdcState::new()),
        64,
    );

    let usb = builder.build();

    spawner.spawn(usb_task(usb)).unwrap();
    spawner.spawn(cdc_task(cdc)).unwrap();

    // ================================
    // SIGNAL OUTPUT
    // ================================
    let _pins = [
        Output::new(p.PA0, Level::Low, Speed::Low),
        Output::new(p.PA1, Level::Low, Speed::Low),
        Output::new(p.PA2, Level::Low, Speed::Low),
        Output::new(p.PA3, Level::Low, Speed::Low),
        Output::new(p.PA4, Level::Low, Speed::Low),
        Output::new(p.PA5, Level::Low, Speed::Low),
        Output::new(p.PA6, Level::Low, Speed::Low),
        Output::new(p.PA7, Level::Low, Speed::Low),
    ];

    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM2);
    let regs = tim.regs_gp16();
    regs.cr2().modify(|w| w.set_ccds(
        embassy_stm32::pac::timer::vals::Ccds::ONUPDATE));
    regs.dier().modify(|w| w.set_ude(true));
    tim.set_frequency(Hertz(PDM_FREQUENCY));

    let start_pdm = || unsafe {
        let mut opts = TransferOptions::default();
        opts.circular = true;

        let dma = embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH2);
        let req = embassy_stm32::timer::UpDma::request(&dma);

        tim.reset();

        let t = Transfer::new_write(
            dma,
            req,
            &PDM_SIGNAL,
            embassy_stm32::pac::GPIOA.bsrr().as_ptr() as *mut u32,
            opts,
        );

        tim.start();
        t
    };

    // ================================
    // ADC SETUP
    // ================================
    let _adc = adc::Adc::new(p.ADC1);
    let adc = embassy_stm32::pac::ADC1;

    adc.cr2().modify(|w| {
        w.set_dma(true);
        w.set_cont(true);
    });
    adc.sqr1().modify(|w| w.set_l(0));

    let mut pb1 = Flex::new(p.PB1);
    pb1.set_as_analog();

    adc.sqr3().modify(|w| w.set_sq(0, 9));
    adc.smpr2().modify(|w| w.set_smp(9, adc::SampleTime::CYCLES41_5));

    let start_adc = |buf| unsafe {
        let dma = embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH1);
        let req = embassy_stm32::adc::RxDma::request(&dma);

        let t = Transfer::new_read(
            dma,
            req,
            embassy_stm32::pac::ADC1.dr().as_ptr() as *mut u16,
            buf,
            TransferOptions::default(),
        );

        embassy_stm32::pac::ADC1.cr2().modify(|w| w.set_adon(true));
        t
    };

    let user_button = Input::new(p.PB14, embassy_stm32::gpio::Pull::None);

    let mut phase_accumulator = PhaseAccumulator::new(0.0, 0.1);
    let distance_per_phase_cycle = 9.4;

    static mut ADC_BUF: [u16; NUM_SAMPLES] = [0; NUM_SAMPLES];
    let mut log_counter: u32 = 0;

    // ================================
    // MAIN LOOP
    // ================================
    loop {
        {
            let buf = unsafe { &mut ADC_BUF[..] };
            let adc_dma = start_adc(buf);
            let mut pdm = start_pdm();
            adc_dma.await;
            pdm.request_stop();
            pdm.await;
        }

        let buf = unsafe { &ADC_BUF[..] };
        let mut sum_s = 0.0f32;
        let mut sum_c = 0.0f32;

        for (i, s) in buf.iter().enumerate() {
            let (sn, cs) = SINE_COSINE_TABLE[i];
            sum_s += *s as f32 * sn;
            sum_c += *s as f32 * cs;
        }

        let phase = sum_s.atan2(sum_c);
        phase_accumulator.update(phase);

        if user_button.is_low() {
            phase_accumulator.unwrapped_phase = 0.0;
        }

        log_counter += 1;
        if log_counter % 50 == 0 {
            let pos = phase_accumulator.unwrapped_phase
                * (distance_per_phase_cycle / (2.0 * core::f32::consts::PI));

            info!("Position: {}mm Phase: {}", pos, phase);

            let mut s: String<128> = String::new();
            let _ = core::fmt::write(
                &mut s,
                format_args!("Position: {:.3}mm Phase: {:.3}\r\n", pos, phase),
            );
            let _ = USB_QUEUE.lock().await.borrow_mut().enqueue(s);
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}
