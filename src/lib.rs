#![cfg_attr(not(test), no_std)]
//! **The Synopsys `DesignWare` Ethernet `QoS` controller, as pure logic**, with the Motorcomm
//! `YT8531` physical layer device.
//!
//! The crate knows no system-on-chip and reads no device tree. The port's register window, its
//! bus configuration ([`network_interface_controller::BusConfiguration`]) and the physical layer
//! device's settings ([`motorcomm_physical_layer_device::Settings`]) come from the caller, in
//! [`bring_up::Inputs`].
//!
//! Everything the driver's two halves compute and nothing either touches. A kernel half brings the
//! controller and its physical layer device up, measures direct memory access (DMA) coherence
//! and starts the DMA; a network-stack half fills descriptors and moves the tail pointers. Adapted
//! from OpenBSD's `dwqe` (`sys/dev/ic/dwqe.c`, `dwqereg.h`, `sys/dev/fdt/if_dwqe_fdt.c`, whose
//! JH7110 path reads `starfive,tx-use-rgmii-clk`) `ytphy` (`sys/dev/mii/ytphy.c`) and
//! `ukphy_status` (`sys/dev/mii/ukphy_subr.c`); FreeBSD's `eqos` was a cross-check, and GPL sources
//! gave hardware facts only, no code. Those were Linux's `stmmac` and `dwmac-starfive.c`,
//! `include/uapi/linux/mii.h`, `include/linux/stmmac.h` and `drivers/net/phy/motorcomm.c`.

//! The Internet Systems Consortium (ISC) license requires this notice:
//!
//! ```text
//! SPDX-License-Identifier: ISC
//!
//! Copyright (c) 2008, 2019 Mark Kettenis <kettenis@openbsd.org>
//! Copyright (c) 2017, 2022 Patrick Wildt <patrick@blueri.se>
//! Copyright (c) 2001 Theo de Raadt
//! Copyright (c) 2023 Mark Kettenis <kettenis@openbsd.org>
//!
//! Permission to use, copy, modify, and distribute this software for any
//! purpose with or without fee is hereby granted, provided that the above
//! copyright notice and this permission notice appear in all copies.
//!
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
//! WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
//! ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
//! WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
//! ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
//! OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//! ```

//! `ukphy_subr.c` (`openbsd/src@21ea132db7eac1d478645cd6150c5787dc366ebc`, fetched 2026-11-01 UTC)
//! adds the NetBSD Foundation's BSD-2-Clause notice, which the ISC notice does not cover:
//!
//! ```text
//! SPDX-License-Identifier: BSD-2-Clause
//!
//! Copyright (c) 1998 The NetBSD Foundation, Inc.
//! All rights reserved.
//!
//! This code is derived from software contributed to The NetBSD Foundation
//! by Jason R. Thorpe of the Numerical Aerospace Simulation Facility,
//! NASA Ames Research Center, and by Frank van der Linden.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions
//! are met:
//! 1. Redistributions of source code must retain the above copyright
//!    notice, this list of conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright
//!    notice, this list of conditions and the following disclaimer in the
//!    documentation and/or other materials provided with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE NETBSD FOUNDATION, INC. AND CONTRIBUTORS
//! ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
//! TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
//! PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION OR CONTRIBUTORS
//! BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//! ```

//! # BUGS
//!
//! - Nothing here has touched the device; every test runs against `simulator`. One receive and one
//!   transmit queue, polled: no interrupt, offloads, jumbo frames or multicast.
#[cfg(test)]
extern crate std;

pub mod bring_up;
pub mod cache_coherence;
pub mod link;
/// The README's example, compiled by `cargo test` so it cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

#[doc(alias = "mdio")]
pub mod management_data_io;
pub mod motorcomm_physical_layer_device;
pub mod network_interface_controller;
pub mod registers;
#[cfg(test)]
mod simulator;

/// Offsets inside the DMA region, which the kernel half allocates contiguous: `e1000e`'s layout,
/// because the two network interface controllers' data planes have the same shape.
pub mod layout;

#[cfg(test)]
mod tests;

mod data_plane;
mod descriptor;
mod hardware;

pub use data_plane::{DataPlane, DirectMemoryAccessRegion, Tails, TransmitError};
use descriptor::*;
pub use descriptor::{
    BUFFER_BYTES, DESCRIPTOR_BYTES, MAXIMUM_FRAME_BYTES, RECEIVE_ENTRIES, TRANSMIT_ENTRIES,
};
pub use hardware::Hardware;

#[cfg(kani)]
mod proofs;
