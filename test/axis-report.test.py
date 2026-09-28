"""Offline helper checks; importing the example must not open a GUI."""
import runpy
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

module = runpy.run_path(str(Path(__file__).resolve().parent.parent / "examples" / "axis-report.py"))


class ReportHelperTest(unittest.TestCase):
    def test_launches_separate_process_without_shell(self):
        root = Mock()
        cli = str(Path("checkout with spaces/bin/nextnc.js").resolve())
        module["install"](root, cli)
        self.assertEqual(root.bind.call_args.args[0], "<Control-Alt-n>")
        with patch("subprocess.Popen") as launch:
            root.bind.call_args.args[1]()
            self.assertEqual(launch.call_args.args[0][-2:], ["--choose", cli])
            self.assertNotIn("shell", launch.call_args.kwargs)
            self.assertTrue(launch.call_args.kwargs["start_new_session"])

    def test_report_arguments_preserve_literal_paths(self):
        with patch("subprocess.run") as run:
            module["render"]("bin/nextnc.js", "a '; name.json", "new report.html")
            args = run.call_args.args[0]
            self.assertEqual(args[2], "report")
            self.assertEqual(args[3], str(Path("a '; name.json").resolve()))
            self.assertNotIn("shell", run.call_args.kwargs)


if __name__ == "__main__":
    unittest.main()
