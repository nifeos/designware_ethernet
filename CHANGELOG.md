# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## 0.1.0-pre.1

- First version.
- Breaking: the `jh7110` module is gone, with `Discovered`, `SystemController`, the compatible
  strings, addresses, interface-select register and `BUS_CONFIGURATION` of that system-on-chip, and
  so is `Settings::FALLBACK_GMAC0`. The crate now knows no system-on-chip. `bring_up::Inputs`
  gains `bus_configuration`, which `bring_up` and its coherence probe program, and
  `Settings::from_mainline` is public so a caller can decode the mainline device-tree spelling.
- `cache_coherence::Observation::new` builds an observation from its four parts. A caller that
  packs an account into words can now rebuild it. Before, `#[non_exhaustive]` left no way to
  build one.
- Breaking: the crate root re-exports only `DataPlane`, its traits, its error and the ring sizes.
  The descriptor bits, `CRC_BYTES`, the descriptor constructors and `receive_verdict` are private.
  The descriptor format can now change without a major version.
- `DataPlane::is_frame_waiting` says whether `receive` has a descriptor to take. A poll loop no
  longer judges the next descriptor itself.
- `DataPlane::start` puts its three ring cursors back to slot 0. A data plane started again after
  a controller reset now follows the device.
- `configure` and `apply_speed` restore the extended-address port when a step fails, too. They
  return the step's error.
- `Applied` gains `receive_clock_drive_defaulted` and `receive_data_drive_defaulted`. A drive
  current the table lacks still gets the part's default, and the caller can now report it.
- Named constants with OpenBSD's spellings as aliases replace the last bare bits: flow control,
  the jumbo bit and the transmit flush limit.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
