# Optional AXIS report shortcut

Use the ordinary `report` command on any desktop first. The helper is optional and does not install itself, alter AXIS or read controller state. Test this customization in your simulation configuration before choosing to use it elsewhere.

The [documented USER_COMMAND_FILE hook](https://linuxcnc.org/docs/stable/html/gui/axis.html#_user_command_file) runs configuration-specific Python before AXIS is displayed. Add the following to your existing user command file, using your checkout's absolute paths:

```python
import runpy
nextnc_review = runpy.run_path("/home/you/LinuxCNCNext-NC/examples/axis-report.py")
nextnc_review["install"](root_window, "/home/you/LinuxCNCNext-NC/bin/nextnc.js")
```

If you do not already have a user command file, create one in your simulation configuration directory and reference it with `[DISPLAY] USER_COMMAND_FILE = nextnc-user.py`. Preserve any existing hook. No distribution `axis.tcl`, tool table or HAL file needs editing.

Control+Alt+N opens a separate report-selection process, keeping report generation outside AXIS's event loop. Choose a timestamped diagnostic JSON, then a **new** HTML destination. The helper invokes the same report CLI without a shell and opens the resulting local HTML. Existing destinations are refused. Node.js must be on PATH; the helper also uses Python 3 and Tk supplied with AXIS.

The selection is explicit: neither `latest.json` nor a selected archive is assumed to describe AXIS's currently loaded program. Compare the report's input identity, command, fingerprint and candidate G-code SHA-256 before relying on it. This shortcut neither loads nor runs G-code. The helper's process/argument behavior is tested; visual integration with your AXIS version remains a simulation acceptance step.
