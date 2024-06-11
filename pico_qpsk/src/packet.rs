use byte::BytesExt;
use crc_all::CrcAlgo;
use heapless::Vec;
use ieee802154::mac::{
    Address, FooterMode, Frame, FrameContent, FrameSerDesContext, FrameType, FrameVersion, Header, PanId,
    ShortAddress,
};

/// Macro add padding to go from the size of the payload to the size of the entire Physical packet
///
/// Get the max frame size from the maximum packet size in bytes
///
/// Needed as [heapless::Vec] requires a constant capacity and so the [PhysicalFrame] also requires the same
#[macro_export]
macro_rules! to_max_frame_size {
    // SEE RHODE & SCHWARTZ APP NOTE in technical documents

    ($x:expr) => {
        // MAC Frame FCF[2] + SN[1] + Address[4|10] +  Aux Sec. Header[0|5|6|10|14] + payload[MAX_PAYLOAD_SIZE] + FCS/CRC[2]
        2 + 1 + 10 + 14 + $x + 2
        // Phyisical Preamble[4] + SFD[1] + Frame Length[1]
        + 4+1+1
        as usize

    };
}

/// A type to hold the possible errors that occur whe a Physical frame is being constructed or converted to bytes
#[derive(Debug)]
pub enum FrameConstructionError {
    FrameWrite(byte::Error),
    VecLen,
    MacFrameLength,
}

/// Physical packet preamble
const PHY_PREAMBLE: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

/// Start of frame delimiter
const PHY_SFD: u8 = 0xA7;

/// A Physical frame to send over O-QPSK 802.15.4
///
/// A group only contains the mac frame, everything else is generated on conversion to bytes.
#[derive(Debug)]
pub struct PhysicalFrame<'p, const MAX_FRAME_SIZE: usize> {
    mac_frame: Frame<'p>,
}

impl<'p, const MAX_FRAME_SIZE: usize> PhysicalFrame<'p, MAX_FRAME_SIZE> {
    ///
    ///
    /// ### Arguments
    ///
    /// * `sequence_num`: the packet number in the sequence
    /// * `source_id`: Source PAN ID
    /// * `source`: Source address
    /// * `destination_id`:  Destination PAN ID
    /// * `destination`: Destination address
    /// * `payload`: the data to send
    ///
    /// #### returns: Result<[PhysicalFrame], [FrameConstructionError]>
    ///
    /// ### Examples
    ///
    /// ```
    ///  const MAX_PAYLOAD_SIZE: usize = 8;
    ///     let frame: PhysicalFrame<{ to_max_frame_size!(MAX_PAYLOAD_SIZE) }> = PhysicalFrame::new(
    ///         1,
    ///         PanId(0x4444),        //dest
    ///         ShortAddress(0xABCD), //dest
    ///         PanId(0x2222),        //src
    ///         ShortAddress(0x1234), //src
    ///         &[0x01],
    ///     )
    ///     .unwrap();
    /// ```
    pub fn new(
        sequence_num: u8,
        source_id: PanId,
        source: ShortAddress,
        destination_id: PanId,
        destination: ShortAddress,
        payload: &'p [u8],
    ) -> Result<Self, FrameConstructionError> {
        let no_crc_frame = Frame {
            header: Header {
                // not in packet generator
                ie_present: false, // I don't know what this for
                seq_no_suppress: false,
                // in order of items from Rhode & Schwartz App note excel packet generator
                // type is set by params
                source: Some(Address::Short(source_id, source)),
                version: FrameVersion::Ieee802154_2006,
                destination: Some(Address::Short(destination_id, destination)),
                pan_id_compress: false,
                ack_request: false,
                frame_type: FrameType::Data,
                frame_pending: false,
                auxiliary_security_header: None,
                seq: sequence_num,
            },
            content: FrameContent::Data,
            payload,
            footer: [0x00, 0x00],
        };

        let frame = {
            let v: Vec<_, MAX_FRAME_SIZE> = mac_frame_to_vec(no_crc_frame, FooterMode::None)?;
            const CRC16_KERMIT: CrcAlgo<u16> = CrcAlgo::<u16>::new(0x1021, 16, 0, 0, true);
            let crc = &mut 0u16;
            CRC16_KERMIT.init_crc(crc);
            CRC16_KERMIT.update_crc(crc, &v);
            let mut frame = no_crc_frame;
            frame.footer = crc.to_le_bytes();
            frame
        };

        Ok(PhysicalFrame { mac_frame: frame })
    }

    /// ```
    /// [PREAMBLE][SFD][LEN][----------------MAC PACKET--------------]
    ///                     [FCF][SN][ADDRESS][AUX SEC.][PAYLOAD][FCS]
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8, MAX_FRAME_SIZE>, FrameConstructionError> {
        let mut v: Vec<u8, { MAX_FRAME_SIZE }> = Vec::new();
        v.extend_from_slice(&PHY_PREAMBLE)
            .map_err(|_| FrameConstructionError::VecLen)?;

        let mac_vec: Vec<u8, { MAX_FRAME_SIZE }> = mac_frame_to_vec(self.mac_frame, FooterMode::Explicit)?;

        v.push(PHY_SFD).map_err(|_| FrameConstructionError::VecLen)?;

        let len = u8::try_from(mac_vec.len()).map_err(|_| FrameConstructionError::MacFrameLength)?;

        v.push(len).map_err(|_| FrameConstructionError::VecLen)?;
        v.extend_from_slice(&mac_vec)
            .map_err(|_| FrameConstructionError::VecLen)?;
        Ok(v)
    }
}

/// Convert mac frame to
///
/// ### Arguments
///
/// * `frame`: the mac frame to convert to bytes
/// * `footer_mode`: if a footer should be included (AKA the CRC/FCS)
///
/// #### returns: Result<Vec<u8, { MAX_FRAME_SIZE }>, FrameConstructionError>
///
pub fn mac_frame_to_vec<const MAX_FRAME_SIZE: usize>(
    frame: Frame,
    footer_mode: FooterMode,
) -> Result<Vec<u8, MAX_FRAME_SIZE>, FrameConstructionError> {
    let mut bytes = [0u8; MAX_FRAME_SIZE];
    let mut len = 0usize; // written len
    bytes
        .write_with(&mut len, frame, &mut FrameSerDesContext::no_security(footer_mode))
        .map_err(FrameConstructionError::FrameWrite)?;

    Vec::<u8, MAX_FRAME_SIZE>::from_slice(&bytes[0..len]).map_err(|_| FrameConstructionError::VecLen)
}
