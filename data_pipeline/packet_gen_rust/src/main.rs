use crate::pio_helpers::get_seq_frame_bytes;

mod packet;
mod pio_helpers;

const DEFAULT_PAYLOAD_SIZE: u32 = 4;
const MAX_PAYLOAD_SIZE: usize = 1000;

fn main() {
    println!("Hello, world!");
    let payload_size = DEFAULT_PAYLOAD_SIZE;

    let frame_bytes = get_seq_frame_bytes::<
        MAX_PAYLOAD_SIZE,
        { to_max_frame_size!(MAX_PAYLOAD_SIZE) },
    >(payload_size as usize);
    println!("payload:");
    for frame_byte in frame_bytes {
       
        print!(" 0x{:x},", (frame_byte & 0xf0)>>4);
        print!(" 0x{:x},", frame_byte & 0x0f);

        
        
        // print!(" (0b{:b}",  (frame_byte & 0xf0)>>4);
        // print!(" 0b{:b}),", frame_byte & 0x0f);
        
        
    }
}
