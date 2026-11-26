//! **The rings and their descriptors**: how big each ring and buffer is, the descriptor bits the
//! driver writes and reads, and what a receive descriptor says. The descriptor bits keep
//! OpenBSD's `dwqereg.h` names, and each one's documentation opens with its full name.

pub(crate) const PAGE: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, one page with room to grow.
pub const RECEIVE_ENTRIES: u16 = 16;

/// Transmit descriptors.
pub const TRANSMIT_ENTRIES: u16 = 16;

/// Bytes per descriptor: four little-endian words.
pub const DESCRIPTOR_BYTES: u64 = 16;

/// Bytes per buffer. Written into the receive control register's buffer size field, so the
/// device never writes past it, and large enough that a 1518-byte frame never spans two.
pub const BUFFER_BYTES: u64 = 2048;

/// The largest frame either direction carries, in bytes: 1500 bytes of payload and the 14-byte
/// header, CRC excluded (the device appends it on transmit, and the driver strips it on receive).
/// This is not the maximum transmission unit, which is the 1500 bytes of payload alone.
pub const MAXIMUM_FRAME_BYTES: usize = 1514;

/// The frame check sequence the device leaves on every received frame.
pub(crate) const CRC_BYTES: usize = 4;

// Receive and transmit, read and write-back formats: descriptor word 3 is the one that carries
// ownership, and the only one the data plane judges.

/// **Receive descriptor word 3, owned by the device** (`RDES3_OWN`, `dwqereg.h`). Set by the
/// driver to hand a descriptor over, cleared by the device's write-back.
#[doc(alias = "RDES3_OWN")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE: u32 = 1 << 31;

/// **Receive descriptor word 3, interrupt on completion** (`RDES3_IC`, `dwqereg.h`). Receive, read
/// format: interrupt on completion. Interrupts are masked at the channel; set anyway so the channel
/// status's receive bit reports completions, which the coherence probe reads.
#[doc(alias = "RDES3_IC")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_INTERRUPT_ON_COMPLETION: u32 = 1 << 30;

/// **Receive descriptor word 3, buffer 1 address valid** (`RDES3_BUF1V`, `dwqereg.h`). Receive,
/// read format: buffer 1's physical address is valid.
#[doc(alias = "RDES3_BUF1V")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_PHYSICAL_ADDRESS_VALID: u32 = 1 << 24;

/// **Receive descriptor word 3, context descriptor** (`RDES3_CTXT`, `dwqereg.h`). Receive,
/// write-back: this is a context descriptor (a timestamp), not a frame.
#[doc(alias = "RDES3_CTXT")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_CONTEXT_DESCRIPTOR: u32 = 1 << 30;

/// **Receive descriptor word 3, first descriptor** (`RDES3_FD`, `dwqereg.h`). Receive, write-back:
/// first descriptor of a frame.
#[doc(alias = "RDES3_FD")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_FIRST_DESCRIPTOR: u32 = 1 << 29;

/// **Receive descriptor word 3, last descriptor** (`RDES3_LD`, `dwqereg.h`). Receive, write-back:
/// last descriptor of a frame.
#[doc(alias = "RDES3_LD")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR: u32 = 1 << 28;

/// **Receive descriptor word 3, error summary** (`RDES3_ES`, `dwqereg.h`). Receive, write-back:
/// error summary.
#[doc(alias = "RDES3_ES")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_ERROR_SUMMARY: u32 = 1 << 15;

/// **Receive descriptor word 3, packet length** (`RDES3_LENGTH`, `dwqereg.h`). Receive, write-back:
/// the frame length including the CRC, valid when [`RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR`] is set.
#[doc(alias = "RDES3_LENGTH")]
pub(crate) const RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK: u32 = 0x7fff;

/// **Transmit descriptor word 2, interrupt on completion** (`TDES2_IC`, `dwqereg.h`). Transmit,
/// read format: interrupt on completion (in word 2).
#[doc(alias = "TDES2_IC")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_2_INTERRUPT_ON_COMPLETION: u32 = 1 << 31;

/// **Transmit descriptor word 2, buffer 1 length** (`TDES2_BUF1_LEN`, `dwqereg.h`). Transmit, read
/// format: buffer 1's length (in word 2).
#[doc(alias = "TDES2_BUF1_LEN")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_BYTES_MASK: u32 = 0x3fff;

/// **Transmit descriptor word 3, first segment** (`TDES3_FS`, `dwqereg.h`). Transmit: first
/// descriptor of a frame.
#[doc(alias = "TDES3_FS")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT: u32 = 1 << 29;

/// **Transmit descriptor word 3, last segment** (`TDES3_LS`, `dwqereg.h`). Transmit: last
/// descriptor of a frame.
#[doc(alias = "TDES3_LS")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT: u32 = 1 << 28;

/// **Transmit descriptor word 3, owned by the device** (`TDES3_OWN`, `dwqereg.h`). Transmit: the
/// device owns it ([`RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE`]'s bit, in the transmit ring).
#[doc(alias = "TDES3_OWN")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE: u32 = 1 << 31;

/// **Transmit descriptor word 3, frame length** (`TDES3_FRAME_LEN`, `dwqereg.h`). Transmit, read
/// format: the whole frame's length.
#[doc(alias = "TDES3_FRAME_LEN")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK: u32 = 0x7fff;

/// **Transmit descriptor word 3, error summary** (`TDES3_ES`, `dwqereg.h`). Transmit, write-back:
/// error summary. The driver reclaims a sent descriptor whatever it reports, so only the tests
/// read it, to check the bit is where `dwqereg.h` puts it.
#[cfg(test)]
#[doc(alias = "TDES3_ES")]
pub(crate) const TRANSMIT_DESCRIPTOR_WORD_3_ERROR_SUMMARY: u32 = 1 << 15;

/// **A receive descriptor handed to the device**: the buffer's address in words 0 and 1 (low and
/// high 32 bits, as OpenBSD and Linux's `dwmac4_set_addr` both write it), word 2 clear, and word 3
/// owned, interrupt-on-completion, buffer 1 valid. As two little-endian `u64`s: words 0 and 1,
/// then words 2 and 3.
pub(crate) const fn receive_descriptor_words(buffer_physical_address: u64) -> [u64; 2] {
    [
        buffer_physical_address,
        ((RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
            | RECEIVE_DESCRIPTOR_WORD_3_INTERRUPT_ON_COMPLETION
            | RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_PHYSICAL_ADDRESS_VALID) as u64)
            << 32,
    ]
}

/// The three bits the receive word sets are distinct single bits, so the `|`s are their `^`s,
/// which the compiler checks here; the mutation exemption for `receive_descriptor_words` names
/// this constant. The compiler is the gate's only reader, and it reads at const evaluation, so
/// `dead_code` is allowed here for exactly that reason.
#[allow(dead_code)]
pub(crate) const RECEIVE_DESCRIPTOR_SET_BITS_DO_NOT_OVERLAP: () = assert!(
    RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
        | RECEIVE_DESCRIPTOR_WORD_3_INTERRUPT_ON_COMPLETION
        | RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_PHYSICAL_ADDRESS_VALID
        == RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
            ^ RECEIVE_DESCRIPTOR_WORD_3_INTERRUPT_ON_COMPLETION
            ^ RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_PHYSICAL_ADDRESS_VALID
);

/// **A transmit descriptor for one whole frame in one buffer**: the address, the length with
/// interrupt-on-completion, and word 3 owned, first and last, with the frame length. The device
/// inserts the CRC and pads a short frame (checksum and padding control zero).
pub(crate) const fn transmit_descriptor_words(
    buffer_physical_address: u64,
    length: u16,
) -> [u64; 2] {
    let length = length as u32 & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK;
    let descriptor_word_2 = (length & TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_BYTES_MASK)
        | TRANSMIT_DESCRIPTOR_WORD_2_INTERRUPT_ON_COMPLETION;
    let descriptor_word_3 = TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
        | TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT
        | TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT
        | length;
    [
        buffer_physical_address,
        descriptor_word_2 as u64 | (descriptor_word_3 as u64) << 32,
    ]
}

/// Word 2's length field and interrupt bit share no bit; word 3's three set bits are distinct
/// single bits and none of them reaches the length field; and word 2 sits a whole word below the
/// shifted word 3, for every value either can hold. The compiler checks it here, and the
/// mutation exemption for `transmit_descriptor_words` names this constant. The compiler is the
/// gate's only reader, and it reads at const evaluation, so `dead_code` is allowed here for
/// exactly that reason.
#[allow(dead_code)]
pub(crate) const TRANSMIT_DESCRIPTOR_BITS_DO_NOT_OVERLAP: () = assert!(
    TRANSMIT_DESCRIPTOR_WORD_2_INTERRUPT_ON_COMPLETION
        & TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_BYTES_MASK
        == 0
        && (TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
            | TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT
            | TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT
            == TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
                ^ TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT
                ^ TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT)
        && TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
            & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK
            == 0
        && TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT
            & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK
            == 0
        && TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT
            & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK
            == 0
        && ((u32::MAX as u64) << 32) & (u32::MAX as u64) == 0
);

/// What a receive descriptor's word 3 says, judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum ReceiveVerdict {
    /// The device still owns it.
    NotYet,
    /// A whole, good frame of this many bytes (CRC removed), which fits the buffer and the room.
    Frame(usize),
    /// The device wrote it and it is not a frame to deliver: an error, a context descriptor, a
    /// frame spanning descriptors, or a length it could not have meant. Consumed and reposted.
    Drop,
    /// A whole, error-free frame longer than the caller's room: consumed and reposted like
    /// [`ReceiveVerdict::Drop`], but counted apart, since the caller's buffer was short and the
    /// frame was good.
    NoRoom,
}

/// **Judge a receive descriptor's word 3**, where `room` is how many bytes the caller can take.
///
/// Every field is the device's statement and none is trusted, since a device that lies about a
/// length would lead a trusting driver past its buffer: a `Frame(n)` always has `n <= room`,
/// `n <= MAXIMUM_FRAME_BYTES` and `n + CRC_BYTES <= BUFFER_BYTES`, which the Kani harness below proves for
/// every word.
pub(crate) fn receive_verdict(descriptor_word_3: u32, room: usize) -> ReceiveVerdict {
    if descriptor_word_3 & RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE != 0 {
        return ReceiveVerdict::NotYet;
    }
    let whole = descriptor_word_3
        & (RECEIVE_DESCRIPTOR_WORD_3_FIRST_DESCRIPTOR | RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR)
        == RECEIVE_DESCRIPTOR_WORD_3_FIRST_DESCRIPTOR | RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR;
    if descriptor_word_3 & RECEIVE_DESCRIPTOR_WORD_3_CONTEXT_DESCRIPTOR != 0
        || !whole
        || descriptor_word_3 & RECEIVE_DESCRIPTOR_WORD_3_ERROR_SUMMARY != 0
    {
        return ReceiveVerdict::Drop;
    }
    let wire = (descriptor_word_3 & RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK) as usize;
    let Some(length) = wire.checked_sub(CRC_BYTES) else {
        return ReceiveVerdict::Drop;
    };
    let fits = (14..=MAXIMUM_FRAME_BYTES).contains(&length) && wire as u64 <= BUFFER_BYTES;
    if !fits {
        return ReceiveVerdict::Drop;
    }
    if length > room {
        return ReceiveVerdict::NoRoom;
    }
    ReceiveVerdict::Frame(length)
}

/// First and last are distinct single bits, so the mask and the compared value read the same
/// under `|` and under `^`, which the compiler checks here; the mutation exemption for
/// `receive_verdict` names this constant. The compiler is the gate's only reader, and it reads at
/// const evaluation, so `dead_code` is allowed here for exactly that reason.
#[allow(dead_code)]
pub(crate) const FIRST_AND_LAST_BITS_DO_NOT_OVERLAP: () = assert!(
    RECEIVE_DESCRIPTOR_WORD_3_FIRST_DESCRIPTOR | RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR
        == RECEIVE_DESCRIPTOR_WORD_3_FIRST_DESCRIPTOR ^ RECEIVE_DESCRIPTOR_WORD_3_LAST_DESCRIPTOR
);
