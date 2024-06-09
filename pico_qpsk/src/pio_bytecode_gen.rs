use core::fmt::Display;
use core::iter;
use core::iter::{Chain, FilterMap, FlatMap, Flatten, once, Once, Repeat, Scan, Skip, Take};
use core::str::Chars;

use itertools::{Batching, Itertools, Tuples};


const CHIP_ARRAY: &'static [[u8; 16]] = &[
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

type SwapType<'a> = FlatMap<Tuples<Chars<'a>, (char, char)>, [char; 2], fn((char, char)) -> [char; 2]>;
type ChipSequenceType<'a> = FlatMap<SwapType<'a>, [u8; 16], fn(char) -> [u8; 16]>;
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

fn swap_fn((a, b): (char, char)) -> [char; 2] {
    return [b, a];
}

fn hex_to_chips(c: char) -> [u8; 16] {
    let idx = usize::from(u8::try_from(c.to_digit(16).unwrap()).unwrap());
    let chs = CHIP_ARRAY[idx];
    return chs;
}

fn add_middle(prev: &mut u8, current: u8) -> Option<[u8; 2]> {
    // middle will have q from previous, i from next
    let middle: u8 = (*prev & 0b01) | (current & 0b10);
    *prev = current;
    Some([middle, current])
}

fn chips_to_waves(bit_chip2: u8) -> [Level; 3] {
    match bit_chip2 {
        0b00 => {
            //0000111111110000
            return [Level::Low(4), Level::High(8), Level::Low(4)];
        }
        0b01 => {
            //0000000011111111
            return [Level::Low(8), Level::High(8), Level::Nop];
        }
        0b10 => {
            // 1111111100000000
            return [Level::High(8), Level::Low(8), Level::Nop];
        }
        0b11 => {
            //1111000000001111
            return [Level::High(4), Level::Low(8), Level::High(4)];
        }

        _ => {
            panic!("Illegal bitChip: {bit_chip2}")
        }
    }
}

fn combine_waves(state: &mut Level, next: Level) -> Option<Level> {
    return match next {
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
                    return temp;
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
    };
}

fn levels_to_ints(o: Level) -> Option<u8> {
    return match o {
        Level::Low(v) | Level::High(v) => {
            // the first value could be zero due to how combine_waves_works
            if v > 4 {
                Some(v)
            } else {
                Some(4)
            }
        }
        Level::Nop => None,
    };
}

fn lengths_to_pio_byte_code_ints(len: u8) -> Chain<Take<Repeat<u8>>, Once<u8>> {
    let repeats = usize::from((len - 4) / 2);
    iter::repeat(1u8).take(repeats).chain(iter::once(0u8))
}

fn repeater(repeats: u8, n: u8) -> Take<Repeat<u8>> {
    return iter::repeat(n).take(repeats as usize);
}

pub fn repeat4(n: u8) -> Take<Repeat<u8>> {
    return repeater(4, n);
}
pub fn repeat1(n: u8) -> Take<Repeat<u8>> {
    return repeater(1, n);
}

pub type ConvertType<'a> = Batching<IntsListType<'a>, fn(&mut IntsListType) -> Option<u32>>;

fn pack_bits_into_u32(it: &mut IntsListType) -> Option<u32> {
    let mut value = 0u32;
    let mut bit_idx = 0;
    while let Some(set_bit) = it.next() {
        value |= u32::from(set_bit) << (31 - bit_idx);
        if bit_idx == 31 {
            return Some(value);
        } else {
            bit_idx += 1;
        }
    }
    return None;
}

fn swap(s: &str) -> SwapType {
    s
        // -> swap every other char for endianness
        .chars()
        .into_iter()
        .tuples::<(char, char)>()
        .flat_map(swap_fn)
}

fn get_chip_sequences(s: SwapType) -> ChipSequenceType {
    s.flat_map(hex_to_chips)
}
fn add_middle_bits_for_o_qpsk(cs: ChipSequenceType) -> MiddleBitsType {
    cs.scan(0u8, add_middle as fn(&mut u8, u8) -> Option<[u8; 2]>)
        .flatten()
        // make it align better with existing implemenation for debuging
        .skip(1)
}

pub fn convert(s: &str, repeat_fn: fn(u8) -> Take<Repeat<u8>>) -> ConvertType {
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

    return c2;
    // let iter2 = xs.chars().into_iter();
}
