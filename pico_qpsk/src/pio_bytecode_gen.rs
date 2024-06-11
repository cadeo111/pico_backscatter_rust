use core::iter;
use core::iter::{once, Chain, FilterMap, FlatMap, Flatten, Once, Repeat, Scan, Skip, Take};
use core::slice::Iter;

use itertools::{Batching, Itertools};

const CHIP_ARRAY: &[[u8; 16]] = &[
    [
        0b11, 0b01, 0b10, 0b01, 0b11, 0b00, 0b00, 0b11, 0b01, 0b01, 0b00, 0b10, 0b00, 0b10, 0b11, 0b10,
    ],
    [
        0b11, 0b10, 0b11, 0b01, 0b10, 0b01, 0b11, 0b00, 0b00, 0b11, 0b01, 0b01, 0b00, 0b10, 0b00, 0b10,
    ],
    [
        0b00, 0b10, 0b11, 0b10, 0b11, 0b01, 0b10, 0b01, 0b11, 0b00, 0b00, 0b11, 0b01, 0b01, 0b00, 0b10,
    ],
    [
        0b00, 0b10, 0b00, 0b10, 0b11, 0b10, 0b11, 0b01, 0b10, 0b01, 0b11, 0b00, 0b00, 0b11, 0b01, 0b01,
    ],
    [
        0b01, 0b01, 0b00, 0b10, 0b00, 0b10, 0b11, 0b10, 0b11, 0b01, 0b10, 0b01, 0b11, 0b00, 0b00, 0b11,
    ],
    [
        0b00, 0b11, 0b01, 0b01, 0b00, 0b10, 0b00, 0b10, 0b11, 0b10, 0b11, 0b01, 0b10, 0b01, 0b11, 0b00,
    ],
    [
        0b11, 0b00, 0b00, 0b11, 0b01, 0b01, 0b00, 0b10, 0b00, 0b10, 0b11, 0b10, 0b11, 0b01, 0b10, 0b01,
    ],
    [
        0b10, 0b01, 0b11, 0b00, 0b00, 0b11, 0b01, 0b01, 0b00, 0b10, 0b00, 0b10, 0b11, 0b10, 0b11, 0b01,
    ],
    [
        0b10, 0b00, 0b11, 0b00, 0b10, 0b01, 0b01, 0b10, 0b00, 0b00, 0b01, 0b11, 0b01, 0b11, 0b10, 0b11,
    ],
    [
        0b10, 0b11, 0b10, 0b00, 0b11, 0b00, 0b10, 0b01, 0b01, 0b10, 0b00, 0b00, 0b01, 0b11, 0b01, 0b11,
    ],
    [
        0b01, 0b11, 0b10, 0b11, 0b10, 0b00, 0b11, 0b00, 0b10, 0b01, 0b01, 0b10, 0b00, 0b00, 0b01, 0b11,
    ],
    [
        0b01, 0b11, 0b01, 0b11, 0b10, 0b11, 0b10, 0b00, 0b11, 0b00, 0b10, 0b01, 0b01, 0b10, 0b00, 0b00,
    ],
    [
        0b00, 0b00, 0b01, 0b11, 0b01, 0b11, 0b10, 0b11, 0b10, 0b00, 0b11, 0b00, 0b10, 0b01, 0b01, 0b10,
    ],
    [
        0b01, 0b10, 0b00, 0b00, 0b01, 0b11, 0b01, 0b11, 0b10, 0b11, 0b10, 0b00, 0b11, 0b00, 0b10, 0b01,
    ],
    [
        0b10, 0b01, 0b01, 0b10, 0b00, 0b00, 0b01, 0b11, 0b01, 0b11, 0b10, 0b11, 0b10, 0b00, 0b11, 0b00,
    ],
    [
        0b11, 0b00, 0b10, 0b01, 0b01, 0b10, 0b00, 0b00, 0b01, 0b11, 0b01, 0b11, 0b10, 0b11, 0b10, 0b00,
    ],
];

type SwapType<'a> = FlatMap<Iter<'a, u8>, [u8; 2], fn(&u8) -> [u8; 2]>;
type ChipSequenceType<'a> = FlatMap<SwapType<'a>, [u8; 16], fn(u8) -> [u8; 16]>;
type MiddleBitsType<'a> = Skip<Flatten<Scan<ChipSequenceType<'a>, u8, fn(&mut u8, u8) -> Option<[u8; 2]>>>>;
type RepeatType<'a> = FlatMap<MiddleBitsType<'a>, Take<Repeat<u8>>, fn(u8) -> Take<Repeat<u8>>>;

type LengthsType<'a> = Scan<
    FlatMap<RepeatType<'a>, [Level; 3], fn(u8) -> [Level; 3]>,
    Level,
    fn(&mut Level, Level) -> Option<Level>,
>;

type IntsListType<'a> = Chain<
    Once<u8>,
    FlatMap<
        FilterMap<LengthsType<'a>, fn(Level) -> Option<u8>>,
        Chain<Take<Repeat<u8>>, Once<u8>>,
        fn(u8) -> Chain<Take<Repeat<u8>>, Once<u8>>,
    >,
>;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Level {
    High(u8),
    Low(u8),
    Nop,
}

/// get a swapped version of provided tuple
///
/// ### Arguments
///
/// * `(a, b)`:
///
/// #### returns: [char; 2]
/// [b, a]
///
fn swap_and_split_fn(byte: &u8) -> [u8; 2] {
    let ret: [u8; 2] = [byte & 0b00001111u8, byte >> 4];
    ret
}

///
///
/// ### Arguments
///
/// * `c`: character that is valid hex
///
/// #### returns: [u8; 16]
/// The corresponding chip sequence for the hex character
fn hex_to_chips(c: u8) -> [u8; 16] {
    let idx = usize::from(c);
    CHIP_ARRAY[idx]
}

///
/// This is used to generate the offset part of the offset psk,
/// you cannot go from 11 to 00 you need an intermediary value 01
///
/// [ (1,1) = (i_1,q_1)   (0,0) = (i_2,q_2) ] -> (i_2,q_1) = (0,1) = 0b01
///
///
/// ### Arguments
///
/// * `prev`: the previous 2 bit chip
/// * `current`: the current 2 bit chip
///
/// #### returns: Option<[u8; 2]>
///  return the intermediate chip and the next chip
fn add_middle(prev: &mut u8, current: u8) -> Option<[u8; 2]> {
    // middle will have q from previous, i from next
    let middle: u8 = (*prev & 0b01) | (current & 0b10);
    *prev = current;
    Some([middle, current])
}

/// Turn chips into digital wave forms
///
/// ### Arguments
///
/// * `bit_chip2`: the 2 bit chip
///
/// #### returns: [Level; 3]
///
///
/// basically modeling this 0000111111110000
///
///
///  ie [Level::Low(4), Level::High(8), Level::Low(4)];
///
///
///  ie 4 cycles low, 8 cycles high, 4 cycles low

fn chips_to_waves(bit_chip2: u8) -> [Level; 3] {
    match bit_chip2 {
        0b00 => {
            //0000111111110000
            [Level::Low(4), Level::High(8), Level::Low(4)]
        }
        0b01 => {
            //0000000011111111
            [Level::Low(8), Level::High(8), Level::Nop]
        }
        0b10 => {
            // 1111111100000000
            [Level::High(8), Level::Low(8), Level::Nop]
        }
        0b11 => {
            //1111000000001111
            [Level::High(4), Level::Low(8), Level::High(4)]
        }

        _ => {
            defmt::panic!("Illegal bitChip: {}", bit_chip2)
        }
    }
}

/// Merge waves that are in sequence that are the same level (High or Low)
///
/// ### Arguments
///
/// * `state`: the current High or Low timing
/// * `next`: the next one tp process
///
/// #### returns: [Option<Level>]
///  only returns Some(Level)
///
///  Nop when still adding waves
///
///  High/Low when it changes
///
/// Nop is used so that there is a return value for every time this is called

fn combine_waves(state: &mut Level, next: Level) -> Option<Level> {
    match next {
        Level::High(next_len) => {
            match state {
                Level::High(state_len) => {
                    // if the state is high, and the next one is high,
                    // add them and return nop
                    *state = Level::High(next_len + *state_len);
                    Some(Level::Nop)
                }
                Level::Low(_) => {
                    // if the state is low, and the next one is high,
                    // return the state, and save next as the state
                    let temp = Some(*state);
                    *state = next;
                    temp
                }
                Level::Nop => Some(Level::Nop),
            }
        }
        Level::Low(next_len) => {
            match state {
                Level::Low(state_len) => {
                    // if the state is low, and the next one is low,
                    // add them and return nop
                    *state = Level::Low(next_len + *state_len);
                    Some(Level::Nop)
                }
                Level::High(_) => {
                    // if the state is high, and the next one is low,
                    // return the state, and save next as the state
                    let temp = Some(*state);
                    *state = next;
                    temp
                }
                Level::Nop => Some(Level::Nop),
            }
        }
        Level::Nop => Some(Level::Nop),
    }
}

/// remove state kind wrapper ie High(8) -> 8
///
/// ### Arguments
///
/// * `l`: the level
///
/// #### returns: [Option<u8>]
/// returns None for Nop,
/// Return Some(l) for High/Low
/// Option is used to filter out Nops
fn levels_to_ints(l: Level) -> Option<u8> {
    match l {
        Level::Low(v) | Level::High(v) => {
            // the first value could be zero due to how combine_waves_works
            if v > 4 {
                Some(v)
            } else {
                Some(4)
            }
        }
        Level::Nop => None,
    }
}

/// turn length into a series of bits / pio bytecode
///
/// ### Arguments
///
/// * `len`: the length to translate
///
/// #### returns: [Chain<Take<Repeat<u8>>, Once<u8>>]
///
///
/// ### Examples
///
/// ```
/// lengths_to_pio_byte_code_ints(12) -> 0,1,1,1,1 (this side is first)
///
/// ```
fn lengths_to_pio_byte_code_ints(len: u8) -> Chain<Take<Repeat<u8>>, Once<u8>> {
    let repeats = usize::from((len - 4) / 2);
    iter::repeat(1u8).take(repeats).chain(once(0u8))
}

/// A helper function to repeat a value `n`, `repeat` times
///
/// ### Arguments
///
/// * `repeats`: the number of time to repeat n
/// * `n`: the value to repeat
///
/// #### returns: [Take<Repeat<u8>>]
///
///
/// ### Examples
///
/// ```
/// repeater(8,2) -> 8,8
/// ```
fn repeater(repeats: u8, n: u8) -> Take<Repeat<u8>> {
    iter::repeat(n).take(repeats as usize)
}

/// repeat 4 times,
///
/// necessary for typing reasons, can't return a closure type via a iterator
///
/// ### Arguments
///
/// * `n`: the data to repeat
///
/// #### returns: [Take<Repeat<u8>>]
///
pub fn repeat4(n: u8) -> Take<Repeat<u8>> {
    repeater(4, n)
}

/// repeat 1 times,
///
/// necessary for typing reasons, can't return a closure type via a iterator
///
/// ### Arguments
///
/// * `n`: the data to repeat
///
/// #### returns: Take<Repeat<u8>>
#[allow(dead_code)]
pub fn repeat1(n: u8) -> Take<Repeat<u8>> {
    repeater(1, n)
}

pub type ConvertType<'a> = Batching<IntsListType<'a>, fn(&mut IntsListType) -> Option<u32>>;

/// from an iterator of 0 and 1, pack them into a u32
///
/// ### Arguments
///
/// * `it`: the iterator
///
/// #### returns: [Option<u32>]
///  Some(u32) while there is data in iter
///  None when the iterator is empty
///
/// # Examples
///
/// ```
///  ...0,1,0,1 -> 0b1010..
/// ```
fn pack_bits_into_u32(it: &mut IntsListType) -> Option<u32> {
    let mut value = 0u32;
    let mut bit_idx = 0;
    for set_bit in it.by_ref() {
        value |= u32::from(set_bit) << (31 - bit_idx);
        if bit_idx == 31 {
            return Some(value);
        } else {
            bit_idx += 1;
        }
    }
    // if there isn't a number of bits divisible by 32, make sure that last bit gets saved
    if bit_idx == 0 {
        None
    } else {
        Some(value)
    }
}

/// swap every 2 characters in a string
///
/// # Arguments
///
/// * `s`: the string
///
/// returns: FlatMap<Tuples<Chars, (char, char)>, [char; 2], fn((char, char)) -> [char; 2]>
///
/// # Examples
///
/// ```
/// "ABCD" -> "BADC"
/// ```
fn swap(s: &[u8]) -> SwapType {
    s.iter()
        // -> swap every other char for endianness
        .flat_map(swap_and_split_fn)
}

fn get_chip_sequences(s: SwapType) -> ChipSequenceType {
    s.flat_map(hex_to_chips)
}
fn add_middle_bits_for_o_qpsk(cs: ChipSequenceType) -> MiddleBitsType {
    cs.scan(0u8, add_middle as fn(&mut u8, u8) -> Option<[u8; 2]>)
        .flatten()
        // make it align better with existing implementation for debugging, would work fine without this
        .skip(1)
}

/// Translate a hex string to O-QPSK pio bytecode
///
/// # Arguments
///
/// * `s`: the hex string to translate to pio bytecode
/// * `repeat_fn`: function that defines how many repeats of the waveform there are, see [repeat4]
///
/// returns:  [ConvertType]
///
/// # Examples
///
/// ```
/// let iter = convert(
///         "00000000A71741880B222234124444CDAB0102030405020202090A4B49",
///         repeat4,
///    );
/// ```
pub fn convert(s: &[u8], repeat_fn: fn(u8) -> Take<Repeat<u8>>) -> ConvertType {
    // TODO: make sure there is an even number of characters in s
    // swap for endianness
    let a: SwapType = swap(s);

    // ->  get chip sequences
    let b: ChipSequenceType = get_chip_sequences(a);

    // -> add middle bits for O-QPSK
    let b2: MiddleBitsType = add_middle_bits_for_o_qpsk(b);

    let b3: RepeatType = b2
        // repeat the chips the number of times needed
        .flat_map(repeat_fn);

    let c1: LengthsType = b3
        // translate chips into waves
        .flat_map(chips_to_waves as fn(u8) -> [Level; 3])
        .scan(
            Level::Low(0),
            combine_waves as fn(&mut Level, Level) -> Option<Level>,
        );
    let c1_1: IntsListType = once(0).chain(
        c1.filter_map(levels_to_ints as fn(Level) -> Option<u8>)
            .flat_map(lengths_to_pio_byte_code_ints as fn(u8) -> Chain<Take<Repeat<u8>>, Once<u8>>),
    );
    let c2: ConvertType = c1_1.batching(pack_bits_into_u32 as fn(&mut IntsListType) -> Option<u32>);

    c2
}
