//! **A resolved link, the speed and duplex a controller runs at.**
//!
//! The controller's `mac_configuration` and `start` take it, and a physical layer device
//! module produces it from the standard clause 22 registers. Nothing in it belongs to one vendor's
//! physical layer device, so it lives beside neither.

/// A link speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// 10 Mbit/s.
    Ten,
    /// 100 Mbit/s.
    Hundred,
    /// 1000 Mbit/s.
    Thousand,
}

impl Speed {
    /// Megabits per second.
    pub const fn megabits_per_second(self) -> u32 {
        match self {
            Speed::Ten => 10,
            Speed::Hundred => 100,
            Speed::Thousand => 1000,
        }
    }
}

/// A resolved link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    /// The speed negotiated.
    pub speed: Speed,
    /// Full duplex when true.
    pub full_duplex: bool,
}
