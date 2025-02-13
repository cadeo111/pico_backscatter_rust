use defmt::info;
use embedded_hal::digital::OutputPin;
use heapless::Vec;
use ieee802154::mac::{PanId, ShortAddress};
use itertools::Itertools;
use rp_pico as bsp;
use rp_pico::hal::gpio::bank0::Gpio3;
use rp_pico::hal::gpio::{Function, FunctionPio0, Pin, PinId, PullNone, PullType, ValidFunction};
use rp_pico::hal::pio::{Buffers, PIOExt, ShiftDirection, Tx, SM0};
use rp_pico::pac::RESETS;
use crate::board_setup::ProcessorClockConfig;
use crate::packet::PhysicalFrame;
use crate::pio_bytecode_gen::{convert_advanced, repeat_n, ConvertIterType};
use crate::to_max_frame_size;

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
pub fn initialize_pio<F, PD, P, F2, PD2, PIOS>(
    gpio3: Pin<Gpio3, F, PD>,
    antenna_pin: Pin<P, F2, PD2>,
    pio: PIOS,
    resets: &mut RESETS,
) -> (
    Tx<(PIOS, SM0)>,
    impl FnMut(),
    impl FnMut(StateMachineClockDividerSetting),
)
where
    P: ValidFunction<FunctionPio0>,
    F: Function,
    PD: PullType,
    P: PinId,
    F2: Function,
    PD2: PullType,
    PIOS: PIOExt,
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
    let (mut sm, _, tx) = bsp::hal::pio::PIOBuilder::from_installed_program(installed)
        .set_pins(antenna_pin_id, 1)
        .buffers(Buffers::OnlyTx)
        .autopull(true)
        .pull_threshold(32)
        .out_shift_direction(ShiftDirection::Left)
        .build(sm0);

    sm.set_pindirs([(antenna_pin_id, bsp::hal::pio::PinDir::Output)]);
    info!("PIO setup ok");

    let mut sm = sm.start();

    info!("PIO start ok");

    (
        tx,
        move || {
            start_pin.set_low().unwrap();
        },
        move |setting| match setting {
            StateMachineClockDividerSetting::Fixed {
                integer_part,
                fractional_part,
            } => {
                sm.clock_divisor_fixed_point(integer_part, fractional_part);
            }
            StateMachineClockDividerSetting::Integer(_) => {
                sm.clock_divisor_fixed_point(4, 0);
            }
            StateMachineClockDividerSetting::None => (),
        },
    )
}

#[macro_export]
macro_rules! wave_array {
    ($chip_count:literal) => {{
        use $crate::pio_bytecode_gen::Level;
        const CHIP_COUNT: u8 = $chip_count;
        const QUARTER_CNT: u8 = CHIP_COUNT / 4;
        const HALF_CNT: u8 = CHIP_COUNT / 2;
        const {
            core::assert!(CHIP_COUNT % 4 == 0, "Chip Count must be evenly dividable by 4");
            core::assert!(CHIP_COUNT % 2 == 0, "Chip Count must be evenly dividable by 2");
        };
        [
            [
                Level::Low(QUARTER_CNT),
                Level::High(HALF_CNT),
                Level::Low(QUARTER_CNT),
            ],
            [Level::Low(HALF_CNT), Level::High(HALF_CNT), Level::Nop],
            [Level::High(HALF_CNT), Level::Low(HALF_CNT), Level::Nop],
            [
                Level::High(QUARTER_CNT),
                Level::Low(HALF_CNT),
                Level::High(QUARTER_CNT),
            ],
        ]
    }};
}

#[allow(clippy::enum_variant_names)]
#[allow(dead_code)]
pub enum StandardTransmitOption {
    Clk128MHzOffset8MHz,
    Clk128MHzOffset6MHz,
    Clk128MHzOffset4MHz,
    Clk128MHzOffset2MHz,
}

impl StandardTransmitOption {
    pub fn state_machine_clock(&self) -> StateMachineClockDividerSetting {
        match self {
            StandardTransmitOption::Clk128MHzOffset8MHz => StateMachineClockDividerSetting::None,
            StandardTransmitOption::Clk128MHzOffset2MHz => StateMachineClockDividerSetting::Integer(4),
            StandardTransmitOption::Clk128MHzOffset6MHz => StateMachineClockDividerSetting::None,
            StandardTransmitOption::Clk128MHzOffset4MHz => StateMachineClockDividerSetting::Integer(2),
        }
    }
    pub fn processor_clock(&self) -> ProcessorClockConfig{
        match self {
            StandardTransmitOption::Clk128MHzOffset8MHz => ProcessorClockConfig::F128MHz,
            StandardTransmitOption::Clk128MHzOffset2MHz => ProcessorClockConfig::F128MHz,
            StandardTransmitOption::Clk128MHzOffset6MHz => ProcessorClockConfig::F144MHz,
            StandardTransmitOption::Clk128MHzOffset4MHz => ProcessorClockConfig::F128MHz
        }
    }

    pub fn convert<'a>(&self, message_bytes: &'a [u8]) -> ConvertIterType<'a>{
        match self {
            StandardTransmitOption::Clk128MHzOffset2MHz =>
                convert_advanced(message_bytes, repeat_n::<1>, &wave_array!(16)),
            StandardTransmitOption::Clk128MHzOffset8MHz => 
                convert_advanced(message_bytes, repeat_n::<4>, &wave_array!(16)),
            StandardTransmitOption::Clk128MHzOffset6MHz =>
                convert_advanced(message_bytes, repeat_n::<3>, &wave_array!(24)),
            StandardTransmitOption::Clk128MHzOffset4MHz =>
                convert_advanced(message_bytes, repeat_n::<2>, &wave_array!(16)),
        }
        
    }

}

#[allow(dead_code)]
pub enum StateMachineClockDividerSetting {
    Fixed { integer_part: u16, fractional_part: u8 },
    Integer(u16),
    None,
}



const MAX_PAYLOAD_SIZE: usize = 4;
const MAX_FRAME_SIZE: usize = to_max_frame_size!(MAX_PAYLOAD_SIZE);
pub fn get_testing_generated_frame_bytes() -> Vec<u8, MAX_FRAME_SIZE> {
    let payload: [u8; MAX_PAYLOAD_SIZE] = [0x01, 0x02, 0xA, 0xB];
    let frame: PhysicalFrame<MAX_FRAME_SIZE> = PhysicalFrame::new(
        1,
        PanId(0x4444),        // dest
        ShortAddress(0xABCD), // dest
        PanId(0x2222),        // src
        ShortAddress(0x1234), // src
        &payload,
    )
        .unwrap();
    let frame_bytes = frame.to_bytes().unwrap_or_else(|err| {
        defmt::panic!(
            "Failed to convert frame to bytes, this should never happen ERR:{:?}",
            err
        );
    });
    info!("Created frame -> :{=[u8]:#x}", &frame_bytes);
    frame_bytes
}


#[allow(dead_code)]
fn get_hex_string_as_bytes<const MAX_VEC_SIZE: usize>(message_str: &str) -> Vec<u8, MAX_VEC_SIZE> {
    let str_iter = message_str
        .chars()
        .map(|c| c.to_digit(16).unwrap().try_into().unwrap())
        .tuples()
        .flat_map(|(a, b): (u8, u8)| [a << 4 | b]);
    Vec::from_iter(str_iter)
}