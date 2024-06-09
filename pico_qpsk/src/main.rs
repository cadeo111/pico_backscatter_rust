#![no_std]
#![no_main]

use bsp::entry;
use bsp::hal::clocks::ClocksManager;
use bsp::hal::fugit::RateExtU32;
use bsp::hal::pio::{Buffers, PIOExt, ShiftDirection};
use bsp::hal::{clocks::Clock, pac, sio::Sio, watchdog::Watchdog};
use bsp::Pins;
use cortex_m::delay::Delay;
use defmt::*;
#[allow(unused_imports)]
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use heapless::Vec;
#[allow(unused_imports)]
use panic_probe as _;
use rp_pico as bsp;
use rp_pico::hal::gpio::bank0::Gpio3;
use rp_pico::hal::gpio::{Function, FunctionPio0, Pin, PinId, PullNone, PullType, ValidFunction};
use rp_pico::hal::pio::{Tx, SM0};
use rp_pico::pac::RESETS;

use crate::data_array::RAW_PIO_PACKET;
use crate::pio_bytecode_gen::{convert, repeat4};

// this allows panic handling

mod data_array;

mod pio_bytecode_gen;

/// Sets the system clock to 128MHz
///
/// # Arguments
///
/// * `pac_watchdog`: pac::Peripherals arg for init
/// * `pac_pll_sys_device`:  pac::Peripherals arg for init
/// * `pac_clocks_block`: pac::Peripherals arg for init
/// * `pac_xosc_dev`: pac::Peripherals arg for init
/// * `pac_pll_usb`: pac::Peripherals arg for init
/// * `pac_resets`: pac::Peripherals arg for init
///
/// returns: ClocksManager
///
/// # Examples
///
/// ```
///     let clocks = setup_clocks(
///         pp.WATCHDOG,
///         pp.PLL_SYS,
///         pp.CLOCKS,
///         pp.XOSC,
///         pp.PLL_USB,
///         &mut pp.RESETS,
///     );
/// ```
fn setup_clocks(
    pac_watchdog: pac::WATCHDOG,
    pac_pll_sys_device: pac::PLL_SYS,
    pac_clocks_block: pac::CLOCKS,
    pac_xosc_dev: pac::XOSC,
    pac_pll_usb: pac::PLL_USB,
    pac_resets: &mut pac::RESETS,
) -> ClocksManager {
    info!("Setting up clocks...");

    // set up custom clock frequency of 128MHz
    // custom clock set up from
    // https://github.com/Neotron-Compute/Neotron-Pico-BIOS/blob/fadb7601d290fd62d8a45424c539dc8c0c93cf93/src/main.rs#L346-L404
    // Referred to as NCC
    // vvv from NCC vvv

    // Reset the DMA engine. If we don't do this, starting from probe-rs
    // (as opposed to a cold-start) is unreliable.
    {
        pac_resets.reset().modify(|_r, w| w.dma().set_bit());
        cortex_m::asm::nop();
        pac_resets.reset().modify(|_r, w| w.dma().clear_bit());
        while pac_resets.reset_done().read().dma().bit_is_clear() {}
    }

    // Needed by the clock setup
    let mut watchdog = Watchdog::new(pac_watchdog);

    // Step 1. Turn on the crystal.
    let xosc = bsp::hal::xosc::setup_xosc_blocking(pac_xosc_dev, bsp::XOSC_CRYSTAL_FREQ.Hz())
        .map_err(|_x| false)
        .unwrap();

    // Step 2. Configure watchdog tick generation to tick over every microsecond.
    watchdog.enable_tick_generation((bsp::XOSC_CRYSTAL_FREQ / 1_000_000) as u8);

    // Step 3. Create a clocks manager.
    let mut clocks = bsp::hal::clocks::ClocksManager::new(pac_clocks_block);

    // Step 4. Set up the system PLL.
    //
    // We take the Crystal Oscillator (=12 MHz) with no divider, and ×128 to
    // give a FOUTVCO of [1536] MHz. This must be in the range 750 MHz - 1600 MHz.
    // The factor of 128 is calculated automatically given the desired FOUTVCO.
    //
    // Next we ÷6 on the first post divider to give 256 MHz.
    //
    // Finally we ÷2 on the second post divider to give 128 MHz.
    //
    // We note from the [RP2040
    // Datasheet](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf#page=231),
    // Section 2.18.2.1:
    //
    // > Jitter is minimised by running the VCO at the highest possible
    // > frequency, so that higher post-divide values can be used.

    let pll_sys = bsp::hal::pll::setup_pll_blocking(
        pac_pll_sys_device,
        xosc.operating_frequency(),
        bsp::hal::pll::PLLConfig {
            vco_freq: 1536.MHz(),
            refdiv: 1,
            post_div1: 6,
            post_div2: 2,
        },
        &mut clocks,
        pac_resets,
    )
    .map_err(|_x| false)
    .unwrap();

    // Step 5. Set up a 48 MHz PLL for the USB system.
    let pll_usb = bsp::hal::pll::setup_pll_blocking(
        pac_pll_usb,
        xosc.operating_frequency(),
        bsp::hal::pll::common_configs::PLL_USB_48MHZ,
        &mut clocks,
        pac_resets,
    )
    .map_err(|_x| false)
    .unwrap();
    // Step 6. Set the system to run from the PLLs we just configured.
    clocks
        .init_default(&xosc, &pll_sys, &pll_usb)
        .map_err(|_x| false)
        .unwrap();

    info!("Clocks OK");

    // ^^^ from NCC ^^^
    return clocks;
}

///
///
/// # Arguments
///
/// * `pac_resets`: pac::Peripherals arg for init
/// * `io_bank0`: pac::Peripherals arg for init
/// * `pads_bank0`: pac::Peripherals arg for init
/// * `system_clock_freq_hz`: the clock frequency you set up
/// * `pac_sio`:
///
/// returns: (Pins, Delay)
///
/// # Examples
///
/// ```
///  let (pins, mut delay) = setup_pins_delay(
///         &mut pp.RESETS,
///         pp.IO_BANK0,
///         pp.PADS_BANK0,
///         clocks.system_clock.freq().to_Hz(),
///         pp.SIO,
///     );
///
///
/// ```
fn setup_pins_delay(
    pac_resets: &mut pac::RESETS,
    io_bank0: pac::IO_BANK0,
    pads_bank0: pac::PADS_BANK0,
    system_clock_freq_hz: u32,
    pac_sio: pac::SIO,
) -> (Pins, Delay) {
    info!("setting up pins and delay...");
    // let mut pp = pac::Peripherals::take().unwrap();
    let pc = pac::CorePeripherals::take().unwrap();

    let sio = Sio::new(pac_sio);

    let delay = Delay::new(pc.SYST, system_clock_freq_hz);

    let pins = Pins::new(io_bank0, pads_bank0, sio.gpio_bank0, pac_resets);
    info!("pins and delay OK");
    return (pins, delay);
}

/// Initialize the PIO block with the OQPSK state machine
/// this PIO program can generate any signal that has at least 4 cycle low and high
/// # Arguments
///
/// * `gpio3`: the trigger pin set in the program, the PIO waits for it to go low before starting to read data
/// * `antenna_pin`: the pin that the PIO should control
/// * `pio`:  which pio to use PIO0 or PIO1
/// * `resets`: idk, required to init
///
/// returns: (Tx<(PIOS, SM0)>, impl FnOnce()+Sized)
/// Tx is the writer to the pio buffer, send the data here
/// the function will start the pio reading data, ie, it pulls pin 3 low
///
/// # Examples
///
/// ```
/// let (mut tx, start_pio_execution) = initialize_pio(pins.gpio3, pins.gpio6, pp.PIO0, &mut pp.RESETS);
/// start_pio_execution();
///
///  while tx.is_full() {}
///   tx.write(0b111101100011);
///
/// ```
fn initialize_pio<F: Function, PD: PullType, P: PinId, F2: Function, PD2: PullType, PIOS: PIOExt>(
    gpio3: Pin<Gpio3, F, PD>,
    antenna_pin: Pin<P, F2, PD2>,
    pio: PIOS,
    resets: &mut RESETS,
) -> (Tx<(PIOS, SM0)>, impl FnOnce() -> ())
where
    P: ValidFunction<FunctionPio0>,
{
    info!("Setting up PIO...");

    // this must start high as the pio starts when it goes low should you want to push some data before starting
    let mut start_pin = gpio3.into_push_pull_output();
    start_pin.set_high().unwrap();

    let (mut pio, sm0, _, _, _) = pio.split(resets);

    // Create a pio program
    let program = pio_proc::pio_asm!(
        "wait 0 pin 3",
        ".wrap_target",
        "set pins 0 [1]",
        "loop1:",
        "out x 1",
        "jmp x-- loop1",
        "set pins, 1 [1]",
        "loop2:",
        "out y 1",
        "jmp y-- loop2",
        ".wrap",
        options(max_program_size = 32) // Optional, defaults to 32
    );

    let installed = pio.install(&program.program).unwrap();
    info!("PIO program install ok");
    // Set gpio25 to pio

    let antenna_pin: Pin<_, FunctionPio0, PullNone> = antenna_pin.reconfigure::<FunctionPio0, PullNone>();

    let antenna_pin_id = antenna_pin.id().num;

    // Build the pio program and set pin both for set and side set!
    // We are running with the default divider which is 1 (max speed)
    let (mut sm, _, mut tx) = bsp::hal::pio::PIOBuilder::from_installed_program(installed)
        .set_pins(antenna_pin_id, 1)
        .buffers(Buffers::OnlyTx)
        .autopull(true)
        .pull_threshold(32)
        .out_shift_direction(ShiftDirection::Left)
        .build(sm0);

    sm.set_pindirs([(antenna_pin_id, bsp::hal::pio::PinDir::Output)]);
    info!("PIO setup ok");

    let sm = sm.start();

    info!("PIO start ok");

    (tx, move || {
        start_pin.set_low().unwrap();
    })
}

#[entry]
fn main() -> ! {
    let mut pp = pac::Peripherals::take().unwrap();

    let clocks = setup_clocks(
        pp.WATCHDOG,
        pp.PLL_SYS,
        pp.CLOCKS,
        pp.XOSC,
        pp.PLL_USB,
        &mut pp.RESETS,
    );

    let (pins, mut delay) = setup_pins_delay(
        &mut pp.RESETS,
        pp.IO_BANK0,
        pp.PADS_BANK0,
        clocks.system_clock.freq().to_Hz(),
        pp.SIO,
    );

    let (mut tx, start_pio_execution) = initialize_pio(pins.gpio3, pins.gpio6, pp.PIO0, &mut pp.RESETS);

    info!("generating packet iterator");
    let iter = convert(
        "00000000A71741880B222234124444CDAB0102030405020202090A4B49",
        repeat4,
    );

    info!("delay 5 sec");
    delay.delay_ms(5000);
    info!("start");
    delay.delay_ms(1);

    start_pio_execution();

    let buffer: Vec<u32, 2000> = Vec::from_iter(iter);

    info!("PIN 3 is low Pin 2 is High");
    let mut idx = 0;
    loop {
        if idx % 20 < 10 {
            info!("sending gen packet...");
            for i in &buffer {
                while tx.is_full() {}
                tx.write(*i);
            }
        } else {
            info!("sending static packet...");
            for i in RAW_PIO_PACKET {
                while tx.is_full() {}
                tx.write(*i);
            }
        }
        info!("delay 1 seconds");
        delay.delay_ms(1000);
        idx += 1;
    }
}

// End of file
