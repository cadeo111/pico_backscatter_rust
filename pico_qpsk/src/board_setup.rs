use crate::usb_serial::init_usb_bus;
use cortex_m::delay::Delay;
use defmt::info;
use rp_pico as bsp;
use rp_pico::hal::clocks::ClocksManager;
use rp_pico::hal::fugit::RateExtU32;
use rp_pico::hal::pll::PLLConfig;
use rp_pico::hal::usb::UsbBus;
use rp_pico::hal::{Clock, Sio, Watchdog};
use rp_pico::pac::{Peripherals, PIO0, RESETS};
use rp_pico::{pac, Pins};
use usb_device::bus::UsbBusAllocator;

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
    processor_clk_config: PLLConfig,
    pac_watchdog: pac::WATCHDOG,
    pac_pll_sys_device: pac::PLL_SYS,
    pac_clocks_block: pac::CLOCKS,
    pac_xosc_dev: pac::XOSC,
    pac_pll_usb: pac::PLL_USB,
    pac_resets: &mut RESETS,
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
    let mut clocks = ClocksManager::new(pac_clocks_block);

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
        processor_clk_config,
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
    clocks
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
    pac_resets: &mut RESETS,
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
    (pins, delay)
}

pub fn setup(
    processor_clk_config: ProcessorClockConfig,
) -> (Pins, Delay, RESETS, UsbBusAllocator<UsbBus>, PIO0) {
    // get the hardware peripherals
    let mut pp = Peripherals::take().unwrap();

    // set up the correct clock speed (128MHz)
    let clocks = setup_clocks(
        processor_clk_config.pll(),
        pp.WATCHDOG,
        pp.PLL_SYS,
        pp.CLOCKS,
        pp.XOSC,
        pp.PLL_USB,
        &mut pp.RESETS,
    );

    // set up GPIO and Delay function
    let (pins, delay) = setup_pins_delay(
        &mut pp.RESETS,
        pp.IO_BANK0,
        pp.PADS_BANK0,
        clocks.system_clock.freq().to_Hz(),
        pp.SIO,
    );

    let bus = init_usb_bus(
        pp.USBCTRL_REGS,
        pp.USBCTRL_DPRAM,
        clocks.usb_clock,
        &mut pp.RESETS,
    );

    (pins, delay, pp.RESETS, bus, pp.PIO0)
}

pub enum ProcessorClockConfig {
    Custom(PLLConfig),
    F128MHz,
    F144MHz,
}

impl ProcessorClockConfig {
    pub fn pll(self) -> PLLConfig {
        match self {
            ProcessorClockConfig::Custom(pll) => pll,
            ProcessorClockConfig::F128MHz => PLLConfig {
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
                //
                // see python script to generate vco values for a specific clock frequency
                // https://github.com/raspberrypi/pico-sdk/blob/95ea6acad131124694cda1c162c52cd30e0aece0/src/rp2_common/hardware_clocks/scripts/vcocalc.py
                vco_freq: 1536.MHz(),
                refdiv: 1,
                post_div1: 6,
                post_div2: 2,
            },
            ProcessorClockConfig::F144MHz => PLLConfig {
                // see comment above ProcessorClockConfig::F128MHz for more info
                // vcocalc.py output:
                // Requested: 144.0 MHz
                // Achieved:  144.0 MHz
                // REFDIV:    1
                // FBDIV:     120 (VCO = 1440.0 MHz)
                // PD1:       5
                // PD2:       2
                vco_freq: 1440.MHz(),
                refdiv: 1,
                post_div1: 5,
                post_div2: 2,
            },
        }
    }
}
