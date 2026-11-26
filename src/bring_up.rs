//! **The port's whole bring-up, in order**: identify the controller, take the station address,
//! configure the physical layer device, reset the controller, run the coherence probe, reset again, program both
//! rings, wait for a link and start at its speed. A [`Port`] supplies the register window, the DMA
//! region and a clock; this module supplies the order and every decision, so the process that
//! drives the silicon and the host tests that drive the simulation run the same code.
//!
//! The clocks, the resets and the board's interface select come first and are not here: they sit
//! in windows the controller does not own. So does everything a board's device tree says, which
//! the caller hands over in [`Inputs`].
//!
//! The one bring-up here, [`bring_up_with_motorcomm_yt8531`], serves a board whose physical layer
//! device is a Motorcomm YT8531 and refuses any other. A board with another device drives the
//! controller with the module functions this one calls; a bring-up that takes any physical layer
//! device can follow as an addition.

use crate::cache_coherence::{self, Observation, Verdict};
use crate::link::{Link, Speed};
use crate::motorcomm_physical_layer_device::{self, Settings, SettingsSource};
use crate::network_interface_controller::{
    self as controller, BusConfiguration, Identity, StationMacAddress,
};
use crate::{
    DataPlane, DirectMemoryAccessRegion, Hardware, Tails, layout, management_data_io, registers,
};

/// **What the bring-up drives**: the controller's register window (the [`Hardware`] it is), its
/// DMA region with the tail registers beside it, and a clock. A kernel half implements it over its
/// mappings of the window and the region; the host tests implement it with `simulator`.
///
/// What an implementation guarantees: the [`Hardware`], [`DirectMemoryAccessRegion`] and
/// [`Tails`] contracts, each for its part. The region [`Port::with_region`] lends is the
/// [`layout::DIRECT_MEMORY_ACCESS_REGION_BYTES`] at [`Inputs::region_physical_address`], and its
/// tails are this controller's channel 0 tail registers. The clock never goes backwards.
///
/// What the crate guarantees: it never holds the region past one [`Port::with_region`] call, and
/// it reads the clock only to bound its waits and to report how long they took, so a coarse clock
/// makes those figures coarse and breaks nothing.
///
/// What may be assumed: what [`Hardware`] may assume, the clocks, resets and interface select
/// already up.
pub trait Port: Hardware {
    /// The CPU's handle on the DMA region.
    type Region<'a>: DirectMemoryAccessRegion
    where
        Self: 'a;
    /// The channel's tail registers.
    type Tails<'a>: Tails
    where
        Self: 'a;
    /// Run `f` with the region and the tail registers, which alias the window and so cannot be
    /// borrowed beside it.
    fn with_region<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Region<'_>, &mut Self::Tails<'_>) -> R,
    ) -> R;
    /// A monotonic clock, in microseconds. It should never go backwards; if it does, the bring-up
    /// reads the step back as no time passed, so a wait runs longer and never ends early.
    fn now_microseconds(&mut self) -> u64;
}

/// **What the board and its tree say about this port**, everything the bring-up needs that the
/// controller cannot tell it. Build one with [`Inputs::new`] and set the optional fields after
/// it; the struct is `#[non_exhaustive]`, so an input added later is not a break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Inputs {
    /// The DMA region's physical base, which descriptors name.
    pub region_physical_address: u64,
    /// How the controller's DMA meets the system bus on this board: the `snps,*` properties of
    /// the port's node and its `snps,axi-config` node. Both the coherence probe and the real
    /// configuration program it.
    pub bus_configuration: BusConfiguration,
    /// The station's media access control (MAC) address, the Institute of Electrical and
    /// Electronics Engineers (IEEE) 802.3 standard's name for it: the tree's `local-mac-address`,
    /// if it has one.
    #[doc(alias = "mac_address")]
    pub station_mac_address: Option<StationMacAddress>,
    /// (`physical_layer_device_bus_address`, the IEEE 802.3 standard's physical layer device address) The physical layer device's MDIO
    /// address.
    #[doc(alias = "phy")]
    #[doc(alias = "phy_address")]
    pub physical_layer_device_bus_address: u8,
    /// The physical layer device's board settings.
    #[doc(alias = "phy")]
    pub physical_layer_device: Settings,
    /// Whether an inconclusive coherence probe ([`Verdict::Inconclusive`]: the loopback frame never
    /// completed, so nothing was measured) lets the bring-up go on as if the platform were
    /// coherent. `false` by default, which refuses it with [`Error::NotCoherent`]; set it only for
    /// a platform known to be coherent whose loopback cannot complete.
    pub accept_inconclusive_coherence: bool,
}

impl Inputs {
    /// The inputs every port needs: the DMA region's physical base, the bus configuration, and the
    /// physical layer device's MDIO address and board settings. The station address starts as
    /// `None`, so the bring-up takes the one the controller's registers hold.
    pub const fn new(
        region_physical_address: u64,
        bus_configuration: BusConfiguration,
        physical_layer_device_bus_address: u8,
        physical_layer_device: Settings,
    ) -> Inputs {
        Inputs {
            region_physical_address,
            bus_configuration,
            station_mac_address: None,
            physical_layer_device_bus_address,
            physical_layer_device,
            accept_inconclusive_coherence: false,
        }
    }
}

/// How long the bring-up waits for the physical layer device to resolve a link after autonegotiation restarts. A
/// gigabit negotiation takes about three seconds; five is slack, not a guess at a slow switch.
pub const LINK_WAIT_MILLISECONDS: u64 = 5000;

/// How often the link wait asks the physical layer device.
const LINK_POLL_MICROSECONDS: u64 = 10_000;

/// Why the bring-up refused the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The controller's version register read as nothing (zero or all ones): its clocks or resets
    /// are not up, or the window is wrong.
    Silent {
        /// What the version register read.
        version: u32,
    },
    /// (`NoStationAddress`, after the IEEE 802.3 standard's station address) The station address
    /// is in neither the tree nor the controller's own registers.
    #[doc(alias = "no_mac_address")]
    NoStationAddress,
    /// An MDIO command did not complete.
    #[doc(alias = "mdio")]
    ManagementDataIo(management_data_io::Error),
    /// The physical layer device is not the `YT8531` these board settings are for.
    UnknownPhysicalLayerDevice {
        /// What it answered.
        id: u32,
    },
    /// A controller sequence refused (a reset that did not complete, a region out of reach).
    Controller(controller::Error),
    /// The coherence probe found device DMA not coherent with the caches.
    NotCoherent(Verdict),
}

/// (`AddressSource`, after the IEEE 802.3 standard's station address) Where the station address
/// came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "mac_address_source")]
#[non_exhaustive]
pub enum AddressSource {
    /// The tree's `local-mac-address`, which U-Boot writes from the board's electrically erasable programmable read-only memory (EEPROM).
    Tree,
    /// The controller's address registers, as firmware left them.
    Registers,
}

/// **Everything the bring-up saw**, filled as far as it got, so a refusal still says where it
/// stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Account {
    /// The controller's version register.
    pub version: u32,
    /// (`direct_memory_access_physical_address_bits`, the databook's `ADDR64` field) The DMA's address
    /// width.
    #[doc(alias = "addr64")]
    pub direct_memory_access_physical_address_bits: u8,
    /// The physical layer device's identifier.
    #[doc(alias = "phy")]
    pub physical_layer_device_id: u32,
    /// The physical layer device's I/O voltage, and which tree spelling its settings came from, once configured.
    #[doc(alias = "phy")]
    pub physical_layer_device: Option<(u32, SettingsSource)>,
    /// (`station_mac_address`, the IEEE 802.3 standard's name for a MAC address) The station address
    /// and where it came from.
    #[doc(alias = "mac_address")]
    pub station_mac_address: Option<(StationMacAddress, AddressSource)>,
    /// How long each software reset took, in microseconds: the probe's, then the real one's.
    pub resets_microseconds: [u64; 2],
    /// The coherence probe's verdict and what it saw.
    pub coherence: Option<(Verdict, Observation)>,
    /// The negotiated link, if one resolved.
    pub link: Option<Link>,
    /// How long the link took to resolve, or how long was waited for it.
    pub link_wait_milliseconds: u64,
}

/// **What a bring-up that succeeded answers.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BroughtUp {
    /// The station address programmed into the controller.
    #[doc(alias = "mac_address")]
    pub station_mac_address: StationMacAddress,
    /// The negotiated link, or `None` when none resolved within [`LINK_WAIT_MILLISECONDS`]: the
    /// rings are then programmed and the controller is not started.
    pub link: Option<Link>,
}

/// **Bring the port up with a Motorcomm YT8531 physical layer device**, recording into `account`
/// as it goes. A device with any other identifier is refused with
/// [`Error::UnknownPhysicalLayerDevice`]. On success the controller runs at
/// the link's speed with both rings programmed and empty, and the answer is the station address
/// and the link; with no link the rings are programmed and the controller is not started.
///
/// # Errors
///
/// The first [`Error`] the sequence meets; `account` holds what it saw until then.
///
/// # Examples
///
/// A caller's [`Port`] reaches the window, the DMA region and a clock; the board's tree supplies
/// the rest. This board takes its station address from the tree and its delays from the vendor
/// tree's register values:
///
/// ```no_run
/// use designware_ethernet::StationMacAddress;
/// use designware_ethernet::bring_up::{
///     Account, Error, Inputs, Port, bring_up_with_motorcomm_yt8531,
/// };
/// use designware_ethernet::management_data_io::ClockRange;
/// use designware_ethernet::motorcomm_physical_layer_device::{
///     PadDriveStrength, Settings, SettingsSource,
/// };
/// use designware_ethernet::network_interface_controller::BusConfiguration;
///
/// fn start_port(port: &mut impl Port, region_physical_address: u64) -> Result<(), Error> {
///     let mut bus = BusConfiguration::new(16, 16);
///     bus.fixed_burst = true;
///     bus.management_data_io_clock_range = ClockRange::From250To300Megahertz;
///     let settings = Settings::new(
///         0xa,
///         false,
///         0xa,
///         PadDriveStrength::Code(6),
///         PadDriveStrength::Code(3),
///         [true, true, true],
///         SettingsSource::VendorTree,
///     );
///     let mut inputs = Inputs::new(region_physical_address, bus, 0, settings);
///     inputs.station_mac_address = StationMacAddress::new([0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c]);
///     let mut account = Account::default();
///     let brought_up = bring_up_with_motorcomm_yt8531(port, &inputs, &mut account)?;
///     if brought_up.link.is_none() {
///         // No cable: the rings are programmed and the controller waits to be started.
///     }
///     Ok(())
/// }
/// ```
pub fn bring_up_with_motorcomm_yt8531(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<BroughtUp, Error> {
    let id = Identity::read(port);
    account.version = id.version;
    account.direct_memory_access_physical_address_bits =
        id.direct_memory_access_physical_address_bits();
    if id.version == 0 || id.version == u32::MAX {
        return Err(Error::Silent {
            version: id.version,
        });
    }

    // The address before the reset, which returns the registers to all ones.
    let station = match inputs.station_mac_address {
        Some(m) => (m, AddressSource::Tree),
        None => controller::station_mac_address(
            port.read(registers::MAC_ADDRESS_0_HIGH_REGISTER),
            port.read(registers::MAC_ADDRESS_0_LOW_REGISTER),
        )
        .map(|m| (m, AddressSource::Registers))
        .ok_or(Error::NoStationAddress)?,
    };
    account.station_mac_address = Some(station);
    let station = station.0;

    bring_up_physical_layer_device(port, inputs, account)?;

    // The PHY keeps its receive clock running from here on, which the reset needs.
    account.resets_microseconds[0] = controller::soft_reset(port).map_err(Error::Controller)?;

    let verdict = probe(port, &id, inputs, station, account)?;
    let accepted = match verdict {
        Verdict::Coherent => true,
        Verdict::Inconclusive => inputs.accept_inconclusive_coherence,
        _ => false,
    };
    if !accepted {
        return Err(Error::NotCoherent(verdict));
    }

    // The probe left the controller in loopback with a used ring: reset it and start clean.
    let _ = controller::stop(port);
    account.resets_microseconds[1] = controller::soft_reset(port).map_err(Error::Controller)?;
    zero(port);
    controller::configure(
        port,
        &id,
        &inputs.bus_configuration,
        inputs.region_physical_address,
        station,
    )
    .map_err(Error::Controller)?;

    let link = wait_for_link(port, inputs, account)?;
    Ok(BroughtUp {
        station_mac_address: station,
        link,
    })
}

/// Identify the physical layer device, refuse any other part, and configure it for the board.
fn bring_up_physical_layer_device(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<(), Error> {
    let device = inputs.physical_layer_device_bus_address;
    account.physical_layer_device_id = management_data_io::physical_layer_device_id(
        &mut management_data_io::Bus::with_clock_range(
            port,
            inputs.bus_configuration.management_data_io_clock_range,
        ),
        device,
    )
    .map_err(Error::ManagementDataIo)?;
    if account.physical_layer_device_id
        != motorcomm_physical_layer_device::MOTORCOMM_YT8531_PHYSICAL_LAYER_DEVICE_ID
    {
        return Err(Error::UnknownPhysicalLayerDevice {
            id: account.physical_layer_device_id,
        });
    }
    let applied = motorcomm_physical_layer_device::configure(
        &mut management_data_io::Bus::with_clock_range(
            port,
            inputs.bus_configuration.management_data_io_clock_range,
        ),
        device,
        &inputs.physical_layer_device,
    )
    .map_err(Error::ManagementDataIo)?;
    account.physical_layer_device = Some((
        applied.voltage_millivolts,
        inputs.physical_layer_device.source,
    ));
    Ok(())
}

/// Restart autonegotiation, wait up to [`LINK_WAIT_MILLISECONDS`] for a link, and start the
/// controller at its speed. `None` when no link came up.
fn wait_for_link(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<Option<Link>, Error> {
    let device = inputs.physical_layer_device_bus_address;
    management_data_io::restart_autonegotiation(
        &mut management_data_io::Bus::with_clock_range(
            &mut *port,
            inputs.bus_configuration.management_data_io_clock_range,
        ),
        device,
    )
    .map_err(Error::ManagementDataIo)?;
    let started = port.now_microseconds();
    let link = loop {
        if let Some(l) = motorcomm_physical_layer_device::read_link(
            &mut management_data_io::Bus::with_clock_range(
                &mut *port,
                inputs.bus_configuration.management_data_io_clock_range,
            ),
            device,
        )
        .map_err(Error::ManagementDataIo)?
        {
            break Some(l);
        }
        if port.now_microseconds().saturating_sub(started) >= LINK_WAIT_MILLISECONDS * 1000 {
            break None;
        }
        port.delay_microseconds(LINK_POLL_MICROSECONDS);
    };
    account.link_wait_milliseconds = port.now_microseconds().saturating_sub(started) / 1000;
    account.link = link;
    if let Some(l) = link {
        motorcomm_physical_layer_device::apply_speed(
            &mut management_data_io::Bus::with_clock_range(
                &mut *port,
                inputs.bus_configuration.management_data_io_clock_range,
            ),
            device,
            &inputs.physical_layer_device,
            l.speed,
        )
        .map_err(Error::ManagementDataIo)?;
        controller::start(port, l, false);
    }
    Ok(link)
}

/// **The coherence probe** (`cache_coherence`'s header): MAC loopback, one frame, the CPU's
/// account against the device's.
fn probe(
    port: &mut impl Port,
    id: &Identity,
    inputs: &Inputs,
    station: StationMacAddress,
    account: &mut Account,
) -> Result<Verdict, Error> {
    let region = inputs.region_physical_address;
    zero(port);
    controller::configure(port, id, &inputs.bus_configuration, region, station)
        .map_err(Error::Controller)?;
    let loopback = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };
    controller::start(port, loopback, true);
    let mut data_plane = DataPlane::new(region);
    // A probe frame is a fixed 128 bytes, so `transmit` cannot refuse it for its length, and the
    // ring was just emptied, so it cannot be full; an error here would leave nothing to observe,
    // which `classify` reports as inconclusive.
    let _ = port.with_region(|r, t| cache_coherence::prepare(&mut data_plane, r, t, station));
    let started = port.now_microseconds();
    while !port.with_region(|r, _| cache_coherence::is_delivered(r))
        && port.now_microseconds().saturating_sub(started) < cache_coherence::WAIT_MICROSECONDS
    {
        port.delay_microseconds(1);
    }
    let status = port.read(registers::CHANNEL_STATUS_REGISTER);
    let o = port.with_region(|r, _| cache_coherence::observe(r, station, status));
    let v = cache_coherence::classify(&o);
    account.coherence = Some((v, o));
    Ok(v)
}

/// Zero the region. A stale descriptor from the probe, or a previous wiring, would otherwise read
/// as a received frame.
fn zero(port: &mut impl Port) {
    port.with_region(|r, _| {
        for offset in (0..layout::DIRECT_MEMORY_ACCESS_REGION_BYTES).step_by(8) {
            r.write_u64(offset, 0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management_data_io::PHYSICAL_LAYER_DEVICE_ID_1_REGISTER;
    use crate::simulator::{
        BUS_CONFIGURATION, Memory, NetworkInterfaceController, PHYSICAL_LAYER_DEVICE_SETTINGS,
    };

    const REGION: u64 = 0x4800_0000;
    const STATION: StationMacAddress =
        StationMacAddress::new([0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c]).unwrap();
    const GIGABIT: Link = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };

    fn inputs() -> Inputs {
        Inputs {
            region_physical_address: REGION,
            bus_configuration: BUS_CONFIGURATION,
            station_mac_address: Some(STATION),
            physical_layer_device_bus_address: 0,
            physical_layer_device: PHYSICAL_LAYER_DEVICE_SETTINGS,
            accept_inconclusive_coherence: false,
        }
    }

    fn port(memory: Memory, link: Option<Link>) -> NetworkInterfaceController {
        let mut p = NetworkInterfaceController::new(REGION, memory);
        p.physical_layer_device.set_link(link);
        p
    }

    /// The whole sequence on a coherent machine: it ends started at the
    /// link's speed, out of loopback, with the probe's verdict and both resets on the account.
    #[test]
    fn a_coherent_port_with_a_link_comes_up_started_and_out_of_loopback() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Ok(BroughtUp {
                station_mac_address: STATION,
                link: Some(GIGABIT)
            })
        );
        assert!(matches!(account.coherence, Some((Verdict::Coherent, _))));
        assert_eq!(
            account.station_mac_address,
            Some((STATION, AddressSource::Tree))
        );
        assert_eq!(
            account.physical_layer_device,
            Some((1800, SettingsSource::Fallback))
        );
        assert_eq!(account.link, Some(GIGABIT));
        let configuration = p.register(registers::MAC_CONFIGURATION_REGISTER);
        assert_ne!(
            configuration & registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
            0
        );
        assert_eq!(
            configuration & registers::MAC_CONFIGURATION_LOOPBACK_MODE,
            0
        );
        // The probe's frame went round the loopback, never onto the wire.
        assert!(p.wire_out().is_empty());
    }

    /// No link: the rings are programmed and the controller is not started, after the whole wait.
    #[test]
    fn a_port_with_no_link_waits_the_bound_and_is_not_started() {
        let mut p = port(Memory::coherent(REGION), None);
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Ok(BroughtUp {
                station_mac_address: STATION,
                link: None
            })
        );
        assert!(account.link_wait_milliseconds >= LINK_WAIT_MILLISECONDS);
        assert_eq!(
            p.register(registers::MAC_CONFIGURATION_REGISTER)
                & registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
            0
        );
    }

    /// A machine whose DMA does not see the CPU's writes is refused at the probe, and the account
    /// says what the probe saw.
    #[test]
    fn a_port_that_cannot_see_the_cpus_writes_is_refused_at_the_probe() {
        let mut p = port(Memory::write_back_unsnooped(REGION), Some(GIGABIT));
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Err(Error::NotCoherent(Verdict::DeviceCannotSeeCpuWrites))
        );
        assert!(account.coherence.is_some());
        assert_eq!(account.link, None);
    }

    /// No tree address and registers reset to all ones: there is no address to take.
    #[test]
    fn with_no_station_named_anywhere_the_port_is_refused_before_the_physical_layer_device() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        let no_tree = Inputs {
            station_mac_address: None,
            ..inputs()
        };
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &no_tree, &mut account),
            Err(Error::NoStationAddress)
        );
        assert_eq!(account.physical_layer_device_id, 0);
    }

    /// Another physical layer device is refused before its registers are written.
    #[test]
    fn another_physical_layer_device_is_refused_and_named() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.physical_layer_device
            .write_raw(PHYSICAL_LAYER_DEVICE_ID_1_REGISTER, 0x1234);
        let mut account = Account::default();
        assert!(matches!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Err(Error::UnknownPhysicalLayerDevice { .. })
        ));
        assert_eq!(account.physical_layer_device, None);
    }

    /// A physical layer device that drives no receive clock leaves the reset hung, and the refusal comes after the
    /// physical layer device was configured, which the account shows.
    #[test]
    fn a_reset_with_no_receive_clock_is_refused_after_the_physical_layer_device() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.stop_receive_clock();
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Err(Error::Controller(controller::Error::ResetTimeout))
        );
        assert!(account.physical_layer_device.is_some());
        assert_eq!(account.coherence, None);
    }

    /// A version register that reads all zeros or all ones is a window nothing answers, and each
    /// is refused on its own, with the version the register held.
    #[test]
    fn a_silent_version_register_is_refused_whether_it_reads_zero_or_all_ones() {
        for version in [0, u32::MAX] {
            let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
            p.set_register(registers::CORE_VERSION_REGISTER, version);
            let mut account = Account::default();
            assert_eq!(
                bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
                Err(Error::Silent { version })
            );
            assert_eq!(account.version, version);
        }
    }

    /// The link wait is reported in milliseconds and is the time spent: a link that is already up
    /// costs about nothing, and one that never comes costs the bound and no more than one poll
    /// beyond it.
    #[test]
    fn the_link_wait_is_reported_in_milliseconds_of_the_time_spent() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account).unwrap();
        assert!(
            account.link_wait_milliseconds < LINK_POLL_MICROSECONDS / 1000,
            "a link already up costs under one poll: {}",
            account.link_wait_milliseconds
        );
        let mut p = port(Memory::coherent(REGION), None);
        let mut account = Account::default();
        bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account).unwrap();
        assert!(account.link_wait_milliseconds >= LINK_WAIT_MILLISECONDS);
        assert!(
            account.link_wait_milliseconds
                <= LINK_WAIT_MILLISECONDS + LINK_POLL_MICROSECONDS / 1000,
            "no more than one poll past the bound: {}",
            account.link_wait_milliseconds
        );
    }

    /// A probe frame that never comes back costs the probe its whole wait and not a microsecond
    /// more: the wait is the only delay the probe makes, and it stops once the bound is reached.
    #[test]
    fn a_probe_frame_that_never_returns_costs_exactly_the_wait() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.break_loopback();
        let id = Identity::read(&mut p);
        let mut account = Account::default();
        let before = p.now_microseconds();
        probe(&mut p, &id, &inputs(), STATION, &mut account).unwrap();
        assert_eq!(
            p.now_microseconds() - before,
            cache_coherence::WAIT_MICROSECONDS
        );
        assert!(account.coherence.is_some());
    }

    /// Every MDIO command the bring-up sends carries the clock range the bus configuration names,
    /// not the default.
    #[test]
    fn the_bring_up_drives_mdio_at_the_configured_clock_range() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account).unwrap();
        let command = p.register(registers::MAC_MANAGEMENT_DATA_IO_BUS_ADDRESS_REGISTER);
        assert_eq!(
            (command >> management_data_io::CLOCK_RANGE_SHIFT)
                & management_data_io::CLOCK_RANGE_FIELD_MASK,
            BUS_CONFIGURATION.management_data_io_clock_range.field()
        );
        assert_ne!(
            BUS_CONFIGURATION.management_data_io_clock_range,
            management_data_io::ClockRange::default()
        );
    }

    /// A probe that measured nothing refuses the bring-up unless the inputs accept it: the
    /// default is the stricter answer, and the account holds what the probe saw either way.
    #[test]
    fn an_inconclusive_probe_refuses_the_bring_up_unless_the_inputs_accept_it() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.break_loopback();
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Err(Error::NotCoherent(Verdict::Inconclusive))
        );
        assert!(matches!(
            account.coherence,
            Some((Verdict::Inconclusive, _))
        ));
        assert!(
            !Inputs::new(REGION, BUS_CONFIGURATION, 0, PHYSICAL_LAYER_DEVICE_SETTINGS)
                .accept_inconclusive_coherence
        );

        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.break_loopback();
        let mut accepting = inputs();
        accepting.accept_inconclusive_coherence = true;
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &accepting, &mut account),
            Ok(BroughtUp {
                station_mac_address: STATION,
                link: Some(GIGABIT)
            })
        );
        assert!(matches!(
            account.coherence,
            Some((Verdict::Inconclusive, _))
        ));
    }

    /// The region is zeroed, the whole of it: a word the probe or a previous wiring left behind
    /// would read as a received frame.
    #[test]
    fn the_region_is_zeroed_to_its_last_word() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let last = layout::DIRECT_MEMORY_ACCESS_REGION_BYTES - 8;
        p.with(|r, _| {
            r.write_u64(last, 0xdead_beef);
            r.write_u64(last - 4096, 0xfeed);
        });
        let mut account = Account::default();
        bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account).unwrap();
        let (end, earlier) = p.with(|r, _| (r.read_u64(last), r.read_u64(last - 4096)));
        assert_eq!(end, 0, "the last word");
        assert_eq!(earlier, 0, "a word a page before it");
    }

    /// Both waits measure elapsed time, not the clock's reading: on a machine that has been up
    /// a long while a link that never comes still costs the bound, and the probe still waits for
    /// its frame.
    #[test]
    fn the_waits_measure_elapsed_time_on_a_clock_that_started_long_ago() {
        let mut p = port(Memory::coherent(REGION), None);
        p.set_clock_microseconds(1 << 40);
        let mut account = Account::default();
        assert_eq!(
            bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
            Ok(BroughtUp {
                station_mac_address: STATION,
                link: None
            })
        );
        assert!(matches!(account.coherence, Some((Verdict::Coherent, _))));
        assert!(account.link_wait_milliseconds >= LINK_WAIT_MILLISECONDS);
        assert!(
            account.link_wait_milliseconds
                <= LINK_WAIT_MILLISECONDS + LINK_POLL_MICROSECONDS / 1000
        );
    }

    /// A clock that steps back reads as no time passed, never as a wrapped elapsed time: the probe
    /// still sees its frame, and a link that never comes still costs the whole bound rather than
    /// ending the wait at once (or panicking in a debug build).
    #[test]
    fn a_clock_that_steps_back_never_ends_a_wait_early() {
        for readings in 0..4 {
            let mut p = port(Memory::coherent(REGION), None);
            p.set_clock_microseconds(1 << 20);
            p.step_clock_back(readings, 1000);
            let mut account = Account::default();
            assert_eq!(
                bring_up_with_motorcomm_yt8531(&mut p, &inputs(), &mut account),
                Ok(BroughtUp {
                    station_mac_address: STATION,
                    link: None
                }),
                "a step back at reading {readings}"
            );
            assert!(matches!(account.coherence, Some((Verdict::Coherent, _))));
            assert!(
                account.link_wait_milliseconds >= LINK_WAIT_MILLISECONDS - 1,
                "a step back at reading {readings}: {}",
                account.link_wait_milliseconds
            );
        }
    }
}
