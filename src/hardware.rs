//! **What the data plane needs from the hardware**, so the simulator can stand in for the MAC.

/// **The controller's register window, as a driver reaches it.** A kernel half implements it over
/// its mapping of the window; the host tests implement it with `simulator`. The window is
/// [`registers::REGISTER_WINDOW_BYTES`](crate::registers::REGISTER_WINDOW_BYTES) long, the
/// device tree's `reg`.
///
/// What the crate guarantees: every offset it passes is a constant from
/// [`registers`](crate::registers), a multiple of 4 and below
/// [`registers::REGISTER_WINDOW_BYTES`](crate::registers::REGISTER_WINDOW_BYTES), so an
/// implementation may bound the window and panic on any other offset rather than touch memory it
/// was not given. The crate makes only 32-bit accesses.
///
/// What an implementation guarantees: every register access has a side effect the compiler cannot
/// see. A read answers what the controller holds now, and the controller changes bits on its own:
/// the software reset bit and the MDIO busy bit clear themselves, and the bring-up polls them. A
/// write starts something, a reset or an MDIO command. So [`Hardware::read`] and
/// [`Hardware::write`] are each one volatile access of exactly 32 bits
/// (`core::ptr::read_volatile` or `write_volatile` on a mapping the processor treats as device
/// memory), never elided, served from an earlier read, merged, split or reordered against
/// another access to the window. An implementation over a plain map, as the simulator is, is
/// right only against a simulated controller.
///
/// What may be assumed: the controller's clocks and resets are up, and the board's interface
/// select is written, before the first call. Those sit in windows the controller does not own;
/// [`jh7110`](crate::jh7110) says what they are on that board.
pub trait Hardware {
    /// Read the 32-bit register at `offset`, as one volatile 32-bit load.
    fn read(&mut self, offset: u32) -> u32;
    /// Write `value` to the 32-bit register at `offset`, as one volatile 32-bit store.
    fn write(&mut self, offset: u32, value: u32);
    /// Wait at least `microseconds` microseconds. Waiting longer is always correct; the crate
    /// uses it for its poll intervals and never for a deadline it must meet.
    fn delay_microseconds(&mut self, microseconds: u64);
}
