//! **The data plane**: the regions and tails a driver supplies, and the receive and transmit loop
//! over them.

use super::*;

/// **The DMA region, as the data plane reads and writes it.** A network-stack half implements it
/// over its mapping of the region, a kernel half over the same region during the bring-up's
/// coherence probe; the host tests implement it over a simulated memory. `e1000e`'s
/// `DirectMemoryAccessRegion`, with the same shape and the same contract.
///
/// What the crate guarantees: offsets are bytes from the region's base. Every access lies inside
/// [`layout::DIRECT_MEMORY_ACCESS_REGION_BYTES`]: the Kani harnesses prove each descriptor and
/// buffer offset inside its part of the region and each delivered frame inside its buffer. Every [`DirectMemoryAccessRegion::read_u64`] and
/// [`DirectMemoryAccessRegion::write_u64`] offset is a multiple of 8. An implementation may still
/// bound them (a mapped window, for example, that panics on an out-of-window offset rather than
/// touching memory it was not given).
///
/// What an implementation guarantees: the device writes this memory behind the compiler's back,
/// and reads what the driver writes without the compiler seeing a reader. So every method is a
/// volatile access (`core::ptr::read_volatile` and `write_volatile`, or a copy built from them): a
/// read is never served from an earlier one, and a write is never dropped or merged. `&self` on the
/// reads is a borrow of the mapping, not a promise that the memory does not change. An
/// implementation over a plain slice, as the README's example is, satisfies the signatures and is
/// right only against a simulated device. Ordering against the device is [`Tails::barrier`]'s.
///
/// What may be assumed: the region is physically contiguous at the address
/// [`DataPlane::new`] was given, and device DMA is coherent with the CPU's caches, so an
/// implementation does no cache maintenance. The bring-up measures the second rather than assume
/// it: [`cache_coherence`](crate::cache_coherence) runs once per bring-up, and a machine whose
/// device DMA is not coherent is refused
/// ([`bring_up::Error::NotCoherent`](crate::bring_up::Error::NotCoherent)) before any frame
/// moves.
pub trait DirectMemoryAccessRegion {
    /// Read the little-endian `u64` at `offset`, as one volatile 64-bit load.
    fn read_u64(&self, offset: u64) -> u64;
    /// Write `value` at `offset`, little-endian, as one volatile 64-bit store.
    fn write_u64(&mut self, offset: u64, value: u64);
    /// Copy `out.len()` bytes starting at `offset` into `out`, by volatile loads.
    fn read_bytes(&self, offset: u64, out: &mut [u8]);
    /// Copy `source` to `offset`, by volatile stores.
    fn write_bytes(&mut self, offset: u64, source: &[u8]);
}

/// **The two tail pointer registers, and the ordering between them and the region**: the only
/// register writes the data plane makes. Whoever holds the controller's DMA register page
/// ([`registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE_OFFSET_BYTES`]) implements it: a network-stack
/// half over its mapping of that one page, a kernel half over its mapping of the window during the
/// bring-up; the host tests implement it over the simulator's registers.
///
/// What an implementation guarantees: each tail write is one volatile 32-bit store to channel 0's
/// register, [`registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER`] or
/// [`registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER`]. A tail write is what tells a
/// suspended DMA channel that descriptors are waiting, so none may be dropped, merged or held back
/// to a later store.
///
/// What the crate guarantees: a value is the low 32 bits of the physical address one past the last
/// descriptor the device may use. Configuring the controller refuses a region straddling a 4 GiB
/// boundary
/// ([`network_interface_controller::Error::RegionOutOfReach`](crate::network_interface_controller::Error::RegionOutOfReach)),
/// so the high bits the controller holds from the ring's base are the same for every value.
///
/// [`Tails::barrier`] orders the region against the device on every architecture the crate builds
/// for: on `x86_64` a compiler fence (`core::sync::atomic::compiler_fence`) is enough, since the
/// processor keeps stores in order and loads in order; on `aarch64` it is `dmb osh` (Linux's
/// `dma_wmb` and `dma_rmb` are its store and load halves, `dmb oshst` and `dmb oshld`); on
/// `riscv64`, the JH7110's architecture, it is `fence iorw, iorw` (Linux orders a store before a
/// register write with `fence w, o` and two loads with `fence r, r`).
pub trait Tails {
    /// Write the receive tail pointer's low 32 bits
    /// ([`registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER`]), as one volatile 32-bit
    /// store.
    fn set_receive_tail(&mut self, tail_pointer_low_bits: u32);
    /// Write the transmit tail pointer's low 32 bits
    /// ([`registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER`]), as one volatile 32-bit
    /// store.
    fn set_transmit_tail(&mut self, tail_pointer_low_bits: u32);
    /// Order every region access before this against every access after it, as seen by the
    /// device. Called between filling a descriptor and the tail write that announces it, and
    /// between reading an ownership bit and reading the bytes it covers.
    fn barrier(&mut self);
}

/// Why a frame was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransmitError {
    /// Empty, or longer than [`MAXIMUM_FRAME_BYTES`].
    BadLength,
    /// Every descriptor is still the device's.
    Full,
}

/// **The data plane**: ring cursors and the region's physical base. Everything a network stack does
/// to the device goes through this, so all of it runs on the host against a simulated device.
///
/// A tail pointer is one past the last descriptor the device may use. On receive one descriptor,
/// the one just behind [`DataPlane::receive_cursor`], is always held back, so that "the device has
/// every descriptor" and "the device has none" are never the same tail address; each received frame
/// reposts it and holds back the slot just consumed.
///
/// # Examples
///
/// Post the receive ring, queue a frame, and poll. Here the region is ordinary memory and the
/// tails go nowhere, so nothing answers:
///
/// ```
/// use designware_ethernet::{
///     DataPlane, DirectMemoryAccessRegion, MAXIMUM_FRAME_BYTES, Tails, layout,
/// };
/// # struct Memory(Vec<u8>);
/// # impl DirectMemoryAccessRegion for Memory {
/// #     fn read_u64(&self, at: u64) -> u64 { let mut b = [0; 8]; self.read_bytes(at, &mut b); u64::from_le_bytes(b) }
/// #     fn write_u64(&mut self, at: u64, value: u64) { self.write_bytes(at, &value.to_le_bytes()) }
/// #     fn read_bytes(&self, at: u64, out: &mut [u8]) { out.copy_from_slice(&self.0[at as usize..][..out.len()]) }
/// #     fn write_bytes(&mut self, at: u64, from: &[u8]) { self.0[at as usize..][..from.len()].copy_from_slice(from) }
/// # }
/// # struct NoDevice;
/// # impl Tails for NoDevice {
/// #     fn set_receive_tail(&mut self, _: u32) {}
/// #     fn set_transmit_tail(&mut self, _: u32) {}
/// #     fn barrier(&mut self) {}
/// # }
/// let mut region = Memory(vec![0; layout::DIRECT_MEMORY_ACCESS_REGION_BYTES as usize]);
/// let mut tails = NoDevice;
/// let mut data_plane = DataPlane::new(0x4000_0000);
/// data_plane.start(&mut region, &mut tails);
/// data_plane.transmit(&mut region, &mut tails, &[0xff; 60]).unwrap();
/// let mut frame = [0; MAXIMUM_FRAME_BYTES];
/// assert_eq!(data_plane.receive(&mut region, &mut tails, &mut frame), None);
/// assert_eq!(data_plane.dropped(), 0);
/// ```
#[derive(Debug)]
pub struct DataPlane {
    physical_address: u64,
    receive_next: u16,
    transmit_next: u16,
    transmit_clean: u16,
    dropped: u32,
    dropped_for_room: u32,
}

impl DataPlane {
    /// A data plane over the region at `data_plane_physical_address`. Call [`DataPlane::start`]
    /// first.
    pub fn new(data_plane_physical_address: u64) -> DataPlane {
        DataPlane {
            physical_address: data_plane_physical_address,
            receive_next: 0,
            transmit_next: 0,
            transmit_clean: 0,
            dropped: 0,
            dropped_for_room: 0,
        }
    }

    /// Frames the device wrote that were not delivered, for any reason, wrapping at `u32::MAX`.
    /// It includes [`DataPlane::dropped_for_room`].
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Of [`DataPlane::dropped`], the good frames lost because the caller's `out` was shorter than
    /// the frame, wrapping at `u32::MAX`. It stays zero for a caller that always offers
    /// [`MAXIMUM_FRAME_BYTES`]; the rest of `dropped` is frames the device marked bad or could not
    /// have meant.
    pub fn dropped_for_room(&self) -> u32 {
        self.dropped_for_room
    }

    /// The receive descriptor the next [`DataPlane::receive`] looks at.
    pub fn receive_cursor(&self) -> u16 {
        self.receive_next
    }

    /// The receive tail address for a ring whose held-back slot is `slot`.
    fn receive_tail(&self, slot: u16) -> u32 {
        (self.physical_address + layout::receive_descriptor(slot)) as u32
    }

    /// The transmit tail address when `next` is the next free slot.
    fn transmit_tail(&self, next: u16) -> u32 {
        (self.physical_address + layout::transmit_descriptor(next)) as u32
    }

    /// **Post every receive descriptor but the last and clear the transmit ring.** The kernel
    /// half programmed both ring bases and lengths and started the DMA with both tails at the ring
    /// bases (nothing available); the receive tail write here is what hands the device its
    /// buffers. The three ring cursors go back to slot 0 with the rings, so a second call starts
    /// the same rings over rather than leaving the cursors where the last run stopped; `dropped`
    /// keeps counting.
    pub fn start(&mut self, region: &mut impl DirectMemoryAccessRegion, tails: &mut impl Tails) {
        self.receive_next = 0;
        self.transmit_next = 0;
        self.transmit_clean = 0;
        for index in 0..RECEIVE_ENTRIES {
            let descriptor = layout::receive_descriptor(index);
            let [words_0_and_1, words_2_and_3] = if index == RECEIVE_ENTRIES - 1 {
                [0, 0]
            } else {
                receive_descriptor_words(self.physical_address + layout::receive_buffer(index))
            };
            region.write_u64(descriptor, words_0_and_1);
            region.write_u64(descriptor + 8, words_2_and_3);
        }
        for index in 0..TRANSMIT_ENTRIES {
            let descriptor = layout::transmit_descriptor(index);
            region.write_u64(descriptor, 0);
            region.write_u64(descriptor + 8, 0);
        }
        tails.barrier();
        tails.set_receive_tail(self.receive_tail(RECEIVE_ENTRIES - 1));
    }

    /// Take the next received frame into `out`, returning its length, or `None` when the device has
    /// written nothing new. A descriptor that holds no frame to deliver (an error, a context
    /// descriptor, a frame spanning descriptors, a length it could not have meant) is consumed and
    /// counted in `dropped` and the next tried, so one bad frame cannot stall the ring. One call looks at no more than
    /// [`RECEIVE_ENTRIES`] descriptors, so a device that keeps refilling the ring with error frames
    /// makes it return `None` and the caller polls again.
    ///
    /// `out` must hold [`MAXIMUM_FRAME_BYTES`] for no good frame to be lost. A frame longer than
    /// `out.len()` is consumed and gone, counted in both `dropped` and
    /// [`DataPlane::dropped_for_room`], so a caller can tell its short buffer from a bad frame.
    pub fn receive(
        &mut self,
        region: &mut impl DirectMemoryAccessRegion,
        tails: &mut impl Tails,
        out: &mut [u8],
    ) -> Option<usize> {
        for _ in 0..RECEIVE_ENTRIES {
            let descriptor = layout::receive_descriptor(self.receive_next);
            let descriptor_word_3 = (region.read_u64(descriptor + 8) >> 32) as u32;
            let verdict = receive_verdict(descriptor_word_3, out.len());
            if verdict == ReceiveVerdict::NotYet {
                return None;
            }
            // The ownership bit was read before the bytes it covers, and nothing else stops a
            // weakly ordered CPU reading the buffer first.
            tails.barrier();
            let got = match verdict {
                ReceiveVerdict::Frame(n) => {
                    region.read_bytes(layout::receive_buffer(self.receive_next), &mut out[..n]);
                    Some(n)
                }
                ReceiveVerdict::NoRoom => {
                    self.dropped = self.dropped.wrapping_add(1);
                    self.dropped_for_room = self.dropped_for_room.wrapping_add(1);
                    None
                }
                ReceiveVerdict::Drop | ReceiveVerdict::NotYet => {
                    self.dropped = self.dropped.wrapping_add(1);
                    None
                }
            };
            // Repost the held-back slot behind this one, then hold this one back: its descriptor
            // is the device's write-back and not owned, so leaving it as it is keeps it out of the
            // device's reach until it is reposted in turn.
            let behind = (self.receive_next + RECEIVE_ENTRIES - 1) % RECEIVE_ENTRIES;
            let [words_0_and_1, words_2_and_3] =
                receive_descriptor_words(self.physical_address + layout::receive_buffer(behind));
            let reposted_descriptor = layout::receive_descriptor(behind);
            region.write_u64(reposted_descriptor, words_0_and_1);
            region.write_u64(reposted_descriptor + 8, words_2_and_3);
            tails.barrier();
            tails.set_receive_tail(self.receive_tail(self.receive_next));
            self.receive_next = (self.receive_next + 1) % RECEIVE_ENTRIES;
            if got.is_some() {
                return got;
            }
        }
        None
    }

    /// **Is a frame, or a descriptor to drop, waiting for [`DataPlane::receive`]?** It reads the
    /// next descriptor's word 3 and takes nothing, so a poll loop can skip a sleep. `true` means the
    /// device has written that descriptor back; the frame it holds may still be dropped.
    pub fn is_frame_waiting(&self, region: &impl DirectMemoryAccessRegion) -> bool {
        let descriptor = layout::receive_descriptor(self.receive_next);
        let descriptor_word_3 = (region.read_u64(descriptor + 8) >> 32) as u32;
        receive_verdict(descriptor_word_3, MAXIMUM_FRAME_BYTES) != ReceiveVerdict::NotYet
    }

    /// Queue `frame` for transmission.
    ///
    /// # Errors
    ///
    /// [`TransmitError::BadLength`] when `frame` is empty or longer than [`MAXIMUM_FRAME_BYTES`],
    /// and [`TransmitError::Full`] when every transmit descriptor is still the device's; nothing is
    /// queued either way.
    pub fn transmit(
        &mut self,
        region: &mut impl DirectMemoryAccessRegion,
        tails: &mut impl Tails,
        frame: &[u8],
    ) -> Result<(), TransmitError> {
        if frame.is_empty() || frame.len() > MAXIMUM_FRAME_BYTES {
            return Err(TransmitError::BadLength);
        }
        self.reclaim(region);
        let next = (self.transmit_next + 1) % TRANSMIT_ENTRIES;
        if next == self.transmit_clean {
            return Err(TransmitError::Full);
        }
        region.write_bytes(layout::transmit_buffer(self.transmit_next), frame);
        let descriptor = layout::transmit_descriptor(self.transmit_next);
        let [words_0_and_1, words_2_and_3] = transmit_descriptor_words(
            self.physical_address + layout::transmit_buffer(self.transmit_next),
            frame.len() as u16,
        );
        region.write_u64(descriptor, words_0_and_1);
        // Word 3, with the ownership bit, goes last and in one store with word 2.
        region.write_u64(descriptor + 8, words_2_and_3);
        tails.barrier();
        self.transmit_next = next;
        tails.set_transmit_tail(self.transmit_tail(next));
        Ok(())
    }

    /// Is there a free transmit descriptor right now? Reclaims first.
    pub fn can_transmit(&mut self, region: &mut impl DirectMemoryAccessRegion) -> bool {
        self.reclaim(region);
        (self.transmit_next + 1) % TRANSMIT_ENTRIES != self.transmit_clean
    }

    /// Move the clean cursor past every descriptor the device has handed back.
    fn reclaim(&mut self, region: &impl DirectMemoryAccessRegion) {
        for _ in 0..TRANSMIT_ENTRIES {
            if self.transmit_clean == self.transmit_next {
                break;
            }
            let descriptor_word_3 = (region
                .read_u64(layout::transmit_descriptor(self.transmit_clean) + 8)
                >> 32) as u32;
            if descriptor_word_3 & TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE != 0 {
                break;
            }
            self.transmit_clean = (self.transmit_clean + 1) % TRANSMIT_ENTRIES;
        }
    }
}
