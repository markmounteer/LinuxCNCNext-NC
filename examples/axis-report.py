"""Optional AXIS shortcut. Opens an explicitly selected archive in a separate UI process.

Load with runpy.run_path(), then call install(root_window, absolute_cli_path).
No LinuxCNC imports, controller calls, INI writes or automatic latest-file selection.
"""
from pathlib import Path
import subprocess
import sys


def install(root, cli):
    cli = str(Path(cli).resolve())
    helper = str(Path(cli).parent.parent / "examples" / "axis-report.py")

    def launch(_event=None):
        subprocess.Popen([sys.executable, helper, "--choose", cli], start_new_session=True)
        return "break"

    root.bind("<Control-Alt-n>", launch, add="+")


def render(cli, archive, output):
    return subprocess.run(
        ["node", str(Path(cli).resolve()), "report", str(Path(archive).resolve()), "--output", str(Path(output).resolve())],
        capture_output=True, text=True, timeout=30, check=False,
    )


def choose(cli):
    import tkinter as tk
    from tkinter import filedialog, messagebox
    import webbrowser
    root = tk.Tk()
    root.withdraw()
    try:
        archive = filedialog.askopenfilename(parent=root, title="Choose a Next-NC diagnostic archive (verify its job identity)", filetypes=[("Next-NC archive", "*.json")])
        if not archive:
            return
        output = filedialog.asksaveasfilename(parent=root, title="Save as a NEW Next-NC review file", defaultextension=".html", filetypes=[("HTML report", "*.html")])
        if not output:
            return
        result = render(cli, archive, output)
        if result.returncode:
            messagebox.showerror("Next-NC report failed", result.stderr, parent=root)
            return
        webbrowser.open(Path(output).resolve().as_uri())
    except (OSError, subprocess.SubprocessError) as error:
        messagebox.showerror("Next-NC report failed", str(error), parent=root)
    finally:
        root.destroy()


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] != "--choose":
        raise SystemExit("Usage: axis-report.py --choose /absolute/path/to/bin/nextnc.js")
    choose(sys.argv[2])
