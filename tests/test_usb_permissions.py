"""Exercise the README's actual privileged creation snippet without sudo."""

from pathlib import Path
import os
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class UsbPermissionInstructionsTests(unittest.TestCase):
    def setUp(self):
        readme = (ROOT / "README.md").read_text()
        section = readme.split("### USB permissions\n", 1)[1]
        command = shlex.split(section.split("```bash\n", 1)[1].split("```", 1)[0])
        self.assertEqual(command[:3], ["sudo", "python3", "-c"])
        self.assertEqual(command[4], "/etc/udev/rules.d/70-elgato-foot-pedal.rules")
        self.script = command[3]
        self.rule = (ROOT / "packaging/70-elgato-foot-pedal.rules").read_bytes()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)
        self.target = self.directory / "70-elgato-foot-pedal.rules"

    def create_rule(self):
        return subprocess.run(
            [sys.executable, "-c", self.script, str(self.target)],
            input=self.rule, capture_output=True, timeout=5,
        )

    def test_new_rule_created_then_repeat_refused(self):
        self.assertEqual(self.create_rule().returncode, 0)
        before = self.target.stat()
        self.assertNotEqual(self.create_rule().returncode, 0)
        self.assertEqual(self.target.stat(), before)
        self.assertEqual(self.target.read_bytes(), self.rule)

    def test_existing_unrelated_and_identical_rules_untouched(self):
        for contents in (b"# User or package rule\n", self.rule):
            with self.subTest(contents=contents):
                self.target.write_bytes(contents)
                self.target.chmod(0o640)
                before = self.target.stat()
                result = self.create_rule()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"FileExistsError", result.stderr)
                self.assertEqual(self.target.stat(), before)
                self.assertEqual(self.target.read_bytes(), contents)

    def test_symlinks_including_dangling_are_untouched(self):
        other = self.directory / "other.rules"
        for exists in (False, True):
            with self.subTest(exists=exists):
                if exists:
                    other.write_bytes(b"# Existing target\n")
                self.target.symlink_to(other)
                before = self.target.lstat()
                self.assertNotEqual(self.create_rule().returncode, 0)
                self.assertEqual(self.target.lstat(), before)
                self.assertEqual(other.exists(), exists)
                if exists:
                    self.assertEqual(other.read_bytes(), b"# Existing target\n")
                self.target.unlink()

    def test_special_file_and_directory_refused_without_blocking(self):
        os.mkfifo(self.target)
        before = self.target.lstat()
        self.assertNotEqual(self.create_rule().returncode, 0)
        self.assertEqual(self.target.lstat(), before)
        self.target.unlink()
        self.target.mkdir()
        self.assertNotEqual(self.create_rule().returncode, 0)
        self.assertTrue(self.target.is_dir())


if __name__ == "__main__":
    unittest.main()
