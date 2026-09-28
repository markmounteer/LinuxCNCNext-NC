# Vendored Next-NC format implementation

Source: https://github.com/markmounteer/Fusion360Next-NC

Commit: `9c8f9151573cf36ee6df9cb27cf193600282058f` (v0.1.5).

`part21.js` and `inspect.js` are unchanged copies of `lib/`; `next-nc.js` is the unchanged `src/` writer used to generate synthetic test fixtures. MIT license retained in this directory. No Autodesk code, SDK or private CAM data is included.

The upstream inspector is a bounded format reader, not a machine acceptance check. This repository applies additional strict profile checks in `src/profile.js` before translation. Updates must preserve fixture behavior and pass translation and LinuxCNC interpreter tests.
