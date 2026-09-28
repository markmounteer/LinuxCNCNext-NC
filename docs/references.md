# Primary implementation references

- [LinuxCNC input filters](https://linuxcnc.org/docs/stable/html/gui/filter-programs.html): filename input, G-code stdout, failure status and INI extension setup.
- [LinuxCNC G-codes](https://linuxcnc.org/docs/stable/html/gcode/g-code.html): G8/G18, centre-format arcs, G43, G53, G54..G59.3, G61, G92.1, G94/G95, G96/G97, CSS unit conversion and feedback requirements.
- [LinuxCNC M-codes](https://linuxcnc.org/docs/stable/html/gcode/m-code.html): M2, M3/M4/M5, M6 and coolant semantics.
- [Standalone rs274 interpreter](https://linuxcnc.org/docs/html/man/man1/rs274.1.html): offline batch interpretation and tool/parameter files.
- [Debian bookworm linuxcnc-uspace package](https://packages.debian.org/bookworm/linuxcnc-uspace): isolated CI interpreter dependency; package version is logged by CI.
- [Pinned Next-NC format contract](https://github.com/markmounteer/Fusion360Next-NC/blob/9c8f9151573cf36ee6df9cb27cf193600282058f/docs/format.md): the supported emitted turning profile and its machine-integration boundary.

These references inform the implementation; they do not establish ISO AP238 certification or physical-machine acceptance.

- [LinuxCNC tool compensation and tool tables](https://www.linuxcnc.org/docs/stable/html/gcode/tool-compensation.html): required T/P records, optional offsets/orientation, machine units, toolchanger differences and file/database ownership.
- Suh et al., [STEP-compliant CNC system for turning: Data model, architecture, and implementation](https://doi.org/10.1016/j.cad.2006.02.006), *Computer-Aided Design* 38 (2006), 677–688: authoring/adaptation/execution separation, offline verification and workingstep traceability. See the [bounded application](preflight.md#research-rationale).
