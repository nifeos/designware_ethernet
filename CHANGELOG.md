# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- First version.
- Breaking: the `jh7110` module is gone, with `Discovered`, `SystemController`, the compatible
  strings, addresses, interface-select register and `BUS_CONFIGURATION` of that system-on-chip, and
  so is `Settings::FALLBACK_GMAC0`. The crate now knows no system-on-chip. `bring_up::Inputs`
  gains `bus_configuration`, which `bring_up` and its coherence probe program, and
  `Settings::from_mainline` is public so a caller can decode the mainline device-tree spelling.
- `cache_coherence::Observation::new` builds an observation from its four parts. A caller that
  packs an account into words can now rebuild it. Before, `#[non_exhaustive]` left no way to
  build one.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
