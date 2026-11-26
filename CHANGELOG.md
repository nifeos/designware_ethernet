# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- Breaking: `bring_up::bring_up` is now `bring_up_with_motorcomm_yt8531`, since it brings up that
  one physical layer device. A board with another device drives the controller through the module
  functions, and a generic bring-up can follow as an addition.
- Breaking: `bring_up_with_motorcomm_yt8531` returns `BroughtUp`, the station address it
  programmed and the link it found. Before, a caller could not learn which address the controller
  took.
- Breaking: a station address is a `StationMacAddress`, whose constructor refuses all zeros, all
  ones and a multicast address. `configure`, `Inputs`, `Account` and the coherence probe take
  it, so an unusable address never reaches the controller.
- Breaking: `Link`, `BusConfiguration`, `Settings` and `Inputs` are `#[non_exhaustive]` with a
  `new` constructor, and `Bus::new` builds a `Bus`, so a later field needs no major version. `DataPlane::dropped` is a method.
- Breaking: the bring-up refuses an inconclusive coherence probe unless the caller sets
  `Inputs::accept_inconclusive_coherence`. Before, it went on as if coherent.
- Breaking: `Settings::to_word` and `from_word` are private until the word carries a version.
- `DataPlane::receive` counts a frame longer than `out` in `dropped_for_lack_of_room` as well as
  `dropped`, so a caller can tell its short buffer from a bad frame.
- `management_data_io::ClockRange` and `BusConfiguration::management_data_io_clock_range` set the
  management data input/output (MDIO) clock range; `Bus::with_clock_range` takes one. Before, the
  driver always programmed the range for a 500 to 800 megahertz clock, which stays the default.
- A clock that steps back no longer ends a bring-up wait early: elapsed time saturates at zero.
- Every fallible public function documents its errors; the bring-up, `configure` and `DataPlane`
  carry examples, and the README is the crate page. The README no longer claims a board run.
- The package ships only the source, the licenses, the README and the CHANGELOG, and the manifest
  names its documentation.

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
- `cache_coherence` no longer exports `is_transmit_complete` or `POSTED_RECEIVE_ENTRIES`. Only the
  crate's own tests used them, so the tests now hold their own copies.
- Named constants with OpenBSD's spellings as aliases replace the last bare bits: flow control,
  the jumbo bit and the transmit flush limit.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
