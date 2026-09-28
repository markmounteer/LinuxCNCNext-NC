# Vendored Next-NC format implementation

Source: https://github.com/markmounteer/Fusion360Next-NC

Commit: `cfda07768f09554148db7b2049d3ad608149c91e` (v0.2.0 writer plus shared-reader diagnostic improvements).

`part21.js`, `inspect.js` and `validation-error.js` are unchanged copies of `lib/`; `next-nc.js` is the unchanged `src/` writer used to generate synthetic test fixtures. MIT license retained in this directory. No Autodesk code, SDK or private CAM data is included.

The upstream inspector is a bounded format reader, not a machine acceptance check. This repository applies additional strict profile checks in `src/profile.js` before translation. Updates must preserve fixture behavior and pass translation and LinuxCNC interpreter tests.
