#![no_std]
#![no_main]

use calipertron_core::*;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use embassy_stm32::dma::*;
use embassy_stm32::gpio::{Flex, Input, Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{adc, Config};

use num_traits::Float;

include!(concat!(env!("OUT_DIR"), "/constants.rs"));
const NUM_SAMPLES: usize = SINE_COSINE_TABLE.len();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // ================================
    // CLOCK CONFIG
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

    info!("local_inner_loop start");

    // ================================
    // SIGNAL EMISSION
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

    regs.cr2().modify(|w| {
        w.set_ccds(embassy_stm32::pac::timer::vals::Ccds::ONUPDATE)
    });
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

    adc.cr1().modify(|w| {
        w.set_scan(true);
        w.set_eocie(true);
    });

    adc.cr2().modify(|w| {
        w.set_dma(true);
        w.set_cont(true);
    });

    adc.sqr1().modify(|w| w.set_l(0));

    let mut pb1 = Flex::new(p.PB1);
    pb1.set_as_analog();

    const PIN_CHANNEL: u8 = 9;
    adc.sqr3().modify(|w| w.set_sq(0, PIN_CHANNEL));
    adc.smpr2()
        .modify(|w| w.set_smp(PIN_CHANNEL as usize, adc::SampleTime::CYCLES41_5));

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
        // ---------- DMA PHASE ----------
        {
            let adc_buf = unsafe { &mut ADC_BUF[..] };

            let adc_dma = start_adc(adc_buf);
            let mut pdm_dma = start_pdm();

            adc_dma.await;
            pdm_dma.request_stop();
            pdm_dma.await;
        }

        // ---------- PROCESSING PHASE ----------
        let adc_buf = unsafe { &ADC_BUF[..] };

        let mut sum_sine: f32 = 0.0;
        let mut sum_cosine: f32 = 0.0;

        for (i, sample) in adc_buf.iter().enumerate() {
            let (s, c) = SINE_COSINE_TABLE[i];
            sum_sine += *sample as f32 * s;
            sum_cosine += *sample as f32 * c;

            if (i & 0x0F) == 0 {
                Timer::after(Duration::from_micros(1)).await;
            }
        }

        let phase = sum_sine.atan2(sum_cosine);
        phase_accumulator.update(phase);

        if user_button.is_low() {
            phase_accumulator.unwrapped_phase = 0.0;
        }

        log_counter += 1;
        if log_counter % 50 == 0 {
            info!(
                "Position: {}mm Phase: {}",
                phase_accumulator.unwrapped_phase
                    * (distance_per_phase_cycle / (2.0 * core::f32::consts::PI)),
                phase
            );
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}
