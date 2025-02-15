use crate::board_setup::ProcessorClockConfig;
use crate::packet::PhysicalFrame;
use crate::pio_bytecode_gen::{convert_advanced, ConvertIterType};
use defmt::info;
use embedded_hal::digital::OutputPin;
use heapless::Vec;
use ieee802154::mac::{PanId, ShortAddress};
use itertools::Itertools;
use pio::InstructionOperands::JMP;
use pio::{Instruction, JmpCondition};
use rp_pico as bsp;
use rp_pico::hal::gpio::bank0::Gpio3;
use rp_pico::hal::gpio::{
    Function, FunctionPio0, FunctionSio, Pin, PinId, PullNone, PullType, SioOutput, ValidFunction,
};
use rp_pico::hal::pio::{Buffers, PIOExt, Running, ShiftDirection, StateMachine, Tx, SM0};
use rp_pico::pac::RESETS;

pub struct PioControl<PIOS, PD>
where
    PIOS: PIOExt,
    PD: PullType,
{
    sm: StateMachine<(PIOS, SM0), Running>,
    start_pin: Pin<Gpio3, FunctionSio<SioOutput>, PD>,
}

impl<PIOS, PD> PioControl<PIOS, PD>
where
    PIOS: PIOExt,
    PD: PullType,
{
    pub fn stop(&mut self) {
        self.start_pin.set_high().unwrap();
        self.sm.exec_instruction(Instruction {
            operands: JMP {
                condition: JmpCondition::Always,
                address: 0x1,
            },
            delay: 0,
            side_set: None,
        });
    }
    pub fn start(&mut self) {
        self.start_pin.set_low().unwrap();
    }
    pub fn change_clock_divider(&mut self, setting: StateMachineClockDividerSetting) {
        match setting {
            StateMachineClockDividerSetting::Fixed {
                integer_part,
                fractional_part,
            } => {
                self.sm.clock_divisor_fixed_point(integer_part, fractional_part);
            }
            StateMachineClockDividerSetting::Integer(integer) => {
                self.sm.clock_divisor_fixed_point(integer, 0);
            }
            StateMachineClockDividerSetting::None => (),
        }
    }
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
pub fn initialize_pio<F, PD, P, F2, PD2, PIOS>(
    gpio3: Pin<Gpio3, F, PD>,
    antenna_pin: Pin<P, F2, PD2>,
    pio: PIOS,
    resets: &mut RESETS,
) -> (Tx<(PIOS, SM0)>, PioControl<PIOS, PD>)
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
    let mut start_pin: Pin<Gpio3, FunctionSio<SioOutput>, PD> = gpio3.into_push_pull_output();
    start_pin.set_high().unwrap();

    let (mut pio, sm0, _, _, _) = pio.split(resets);

    // Create a pio program
    // let program = pio_proc::pio_asm!(
    //     ".wrap_target"
    //     "wait 0 pin 3",
    //     "loop:",
    //     "set pins 0 [1]",
    //     "loop1:",
    //     "out x 1",
    //     "jmp x-- loop1",
    //     "set pins, 1 [1]",
    //     "loop2:",
    //     "out y 1",
    //     "jmp y-- loop2",
    //     "jmp loop",
    //     ".wrap"
    //     options(max_program_size = 32) // Optional, defaults to 32
    // );
    let program = pio_proc::pio_asm!(
        "wait 0 pin 3", // not neccesarily required but without, the first high/low length is not determinite
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

    let sm: StateMachine<(PIOS, SM0), Running> = sm.start();
    info!("PIO start ok");

    (tx, PioControl { sm, start_pin })
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
#[derive(Copy, Clone, Debug)]
pub enum StandardTransmitOption {
    Clk128MHzOffset8MHz,
    Clk144MHzOffset6MHz,
    Clk128MHzOffset4MHz,
    Clk128MHzOffset2MHz,
}

impl StandardTransmitOption {
    pub fn state_machine_clock(&self) -> StateMachineClockDividerSetting {
        match self {
            StandardTransmitOption::Clk128MHzOffset8MHz => StateMachineClockDividerSetting::None,
            StandardTransmitOption::Clk128MHzOffset2MHz => StateMachineClockDividerSetting::Integer(4),
            StandardTransmitOption::Clk144MHzOffset6MHz => StateMachineClockDividerSetting::None,
            StandardTransmitOption::Clk128MHzOffset4MHz => StateMachineClockDividerSetting::Integer(2),
        }
    }
    pub fn processor_clock(&self) -> ProcessorClockConfig {
        match self {
            StandardTransmitOption::Clk128MHzOffset8MHz => ProcessorClockConfig::F128MHz,
            StandardTransmitOption::Clk128MHzOffset2MHz => ProcessorClockConfig::F128MHz,
            StandardTransmitOption::Clk144MHzOffset6MHz => ProcessorClockConfig::F144MHz,
            StandardTransmitOption::Clk128MHzOffset4MHz => ProcessorClockConfig::F128MHz,
        }
    }

    pub fn convert<'a>(&self, message_bytes: &'a [u8]) -> ConvertIterType<'a> {
        match self {
            StandardTransmitOption::Clk128MHzOffset2MHz => {
                convert_advanced::<1>(message_bytes, &wave_array!(16))
            }
            StandardTransmitOption::Clk128MHzOffset8MHz => {
                convert_advanced::<4>(message_bytes, &wave_array!(16))
            }
            StandardTransmitOption::Clk144MHzOffset6MHz => {
                convert_advanced::<3>(message_bytes, &wave_array!(24))
            }
            StandardTransmitOption::Clk128MHzOffset4MHz => {
                convert_advanced::<2>(message_bytes, &wave_array!(16))
            }
        }
    }
}

#[allow(dead_code)]
pub enum StateMachineClockDividerSetting {
    Fixed { integer_part: u16, fractional_part: u8 },
    Integer(u16),
    None,
}

/// get a frams to test with a given payload
///
/// # Arguments
///
/// * `payload`:
///
/// returns: Vec<u8, { MAX_FRAME_SIZE }>
///
/// # Examples
///
/// ```
/// const PAYLOAD_SIZE: usize = 4;
/// const MAX_FRAME_SIZE:usize = to_max_frame_size!(PAYLOAD_SIZE);
/// let payload: [u8; PAYLOAD_SIZE] = [0x01, 0x02, 0xA, 0xB];
/// let generated_frame_bytes = &get_testing_generated_frame_bytes::<PAYLOAD_SIZE, MAX_FRAME_SIZE>(&payload);
/// ```
fn get_testing_generated_frame_bytes<const MAX_PAYLOAD_SIZE: usize, const MAX_FRAME_SIZE: usize>(
    payload: &[u8],
) -> Vec<u8, MAX_FRAME_SIZE> {
    assert!(payload.len() <= MAX_PAYLOAD_SIZE, "payload is too big!");

    let frame: PhysicalFrame<MAX_FRAME_SIZE> = PhysicalFrame::new(
        1,
        PanId(0x4444),        // dest
        ShortAddress(0xABCD), // dest
        PanId(0x2222),        // src
        ShortAddress(0x1234), // src
        payload,
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
pub fn get_random_payload_frame_bytes<const MAX_PAYLOAD_SIZE: usize, const MAX_FRAME_SIZE: usize>(
    step: usize,
    size: usize,
) -> Vec<u8, MAX_FRAME_SIZE> {
    assert!(size <= MAX_PAYLOAD_SIZE, "payload is too big!");
    const RANDOMS: [u8; 256] = [
        0x9f, 0xe4, 0xda, 0xa8, 0xcf, 0xd9, 0xf6, 0xc1, 0x34, 0xef, 0xbb, 0x71, 0xce, 0x9e, 0xa5, 0xbe, 0x5e,
        0xb5, 0x56, 0x23, 0x12, 0xde, 0xcb, 0xa9, 0xd5, 0x26, 0xe5, 0xde, 0xfa, 0xc6, 0x4d, 0x61, 0xa8, 0xe7,
        0x92, 0x1c, 0xa1, 0xf9, 0x5f, 0xaa, 0x17, 0xc9, 0xb7, 0xd8, 0x41, 0x17, 0x2c, 0x29, 0xf1, 0x3b, 0x70,
        0x88, 0xfc, 0xa3, 0xaa, 0x3d, 0xcd, 0xb8, 0x2b, 0x7b, 0x8b, 0x39, 0x03, 0xf6, 0x02, 0xed, 0x59, 0x09,
        0x33, 0xb7, 0xf6, 0xa1, 0x7e, 0x8e, 0x4d, 0x5c, 0xf9, 0x0d, 0x12, 0x4a, 0x2e, 0x11, 0x97, 0xd1, 0x9a,
        0x10, 0xdf, 0xc4, 0x84, 0x4c, 0xed, 0x48, 0x12, 0x81, 0xdd, 0xc5, 0xcb, 0x20, 0x68, 0x94, 0x6f, 0xa0,
        0x16, 0x19, 0xad, 0x20, 0x50, 0xb3, 0x0e, 0xef, 0xeb, 0x99, 0xf3, 0xcd, 0x4b, 0xb7, 0x5b, 0xda, 0x9b,
        0xa2, 0xfa, 0x08, 0xc9, 0xe3, 0x6e, 0x97, 0xb2, 0x42, 0xc1, 0x31, 0x9c, 0x88, 0x80, 0x10, 0xfb, 0x59,
        0x5e, 0xd5, 0x38, 0x0e, 0x10, 0x61, 0x7f, 0x84, 0xd0, 0x68, 0x45, 0xc1, 0x25, 0x48, 0x70, 0xf4, 0xa6,
        0x63, 0x7a, 0x4a, 0x65, 0x0b, 0x26, 0x80, 0x46, 0xe7, 0x6c, 0x47, 0x65, 0x65, 0x82, 0x43, 0x96, 0xc6,
        0x32, 0xda, 0xd9, 0x29, 0x81, 0x6f, 0x06, 0x4b, 0x30, 0xc6, 0xc2, 0x60, 0x1d, 0x7a, 0x14, 0xd0, 0xa1,
        0x03, 0x6d, 0x67, 0x75, 0xf4, 0xe0, 0x72, 0x52, 0x07, 0xd8, 0x3e, 0xf5, 0x34, 0x7b, 0x45, 0x62, 0x89,
        0x5f, 0x79, 0xd1, 0xe6, 0xf2, 0x00, 0x00, 0x96, 0xb4, 0xde, 0xfa, 0x88, 0x57, 0x69, 0x8d, 0x4e, 0x6f,
        0x63, 0x0a, 0x90, 0x6f, 0x8d, 0x94, 0x18, 0x15, 0xc2, 0x0b, 0xaa, 0x33, 0xef, 0x9c, 0x4a, 0x6e, 0x2f,
        0xe5, 0x71, 0x4d, 0x85, 0xad, 0x70, 0x95, 0x9f, 0xac, 0x94, 0x78, 0x0d, 0xb5, 0xf4, 0x9e, 0x57, 0xda,
        0xde,
    ];

    let payload_vec: Vec<u8, MAX_PAYLOAD_SIZE> =
        RANDOMS.into_iter().cycle().step_by(step).take(size).collect();

    get_testing_generated_frame_bytes::<MAX_PAYLOAD_SIZE, MAX_FRAME_SIZE>(&payload_vec)
}

pub fn get_seq_frame_bytes<const MAX_PAYLOAD_SIZE: usize, const MAX_FRAME_SIZE: usize>(
    size: usize,
) -> Vec<u8, MAX_FRAME_SIZE> {
    assert!(size <= MAX_PAYLOAD_SIZE, "payload is too big!");
    const SEQ: [u8; 256] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21,
        0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32,
        0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43,
        0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54,
        0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65,
        0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76,
        0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
        0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
        0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9,
        0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA,
        0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB,
        0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC,
        0xDD, 0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED,
        0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
        0xFF,
    ];

    let payload_vec: Vec<u8, MAX_PAYLOAD_SIZE> = SEQ.into_iter().cycle().take(size).collect();

    get_testing_generated_frame_bytes::<MAX_PAYLOAD_SIZE, MAX_FRAME_SIZE>(&payload_vec)
}

// const MAX_PAYLOAD_SIZE: usize = 4;
// const MAX_FRAME_SIZE: usize = to_max_frame_size!(MAX_PAYLOAD_SIZE);

#[allow(dead_code)]
fn get_hex_string_as_bytes<const MAX_VEC_SIZE: usize>(message_str: &str) -> Vec<u8, MAX_VEC_SIZE> {
    let str_iter = message_str
        .chars()
        .map(|c| c.to_digit(16).unwrap().try_into().unwrap())
        .tuples()
        .flat_map(|(a, b): (u8, u8)| [a << 4 | b]);
    Vec::from_iter(str_iter)
}
