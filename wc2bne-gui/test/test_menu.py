import os
import shutil
import subprocess

from test.harness import WindowTest

ENTRY = "applications/io.github.jasenmichael.wc2bne.desktop"
ICON = "icons/hicolor/scalable/apps/io.github.jasenmichael.wc2bne.svg"


class Menu(WindowTest):
    def run_program(self, *args):
        process = self.start(*args)
        output = process.communicate(timeout=10)[0]
        self.processes.remove(process)
        return process.returncode, output

    def data(self, *names):
        return os.path.join(self.home, ".local", "share", *names)

    def test_adds_an_entry_and_icon_that_start_it_from_any_folder(self):
        moved = self.path("my games", "wc2bne-gui")
        shutil.copytree(os.path.dirname(self.program), moved)
        self.program = os.path.join(moved, "wc2bne-gui")
        code, output = self.run_program("--add-to-menu")
        self.assertEqual(code, 0, output)
        with open(self.data(ENTRY), encoding="utf-8") as f:
            entry = f.read()
        self.assertIn('Exec="%s"\n' % self.program, entry)
        self.assertIn("Icon=io.github.jasenmichael.wc2bne\n", entry)
        self.assertIn("Type=Application\n", entry)
        self.assertTrue(os.path.getsize(self.data(ICON)) > 0)
        exec_line = next(line for line in entry.splitlines() if line.startswith("Exec="))
        command = exec_line[len('Exec="'):-1]
        self.assertTrue(os.access(command, os.X_OK))

    def test_adding_again_updates_the_entry(self):
        self.run_program("--add-to-menu")
        moved = self.path("new place", "wc2bne-gui")
        shutil.copytree(os.path.dirname(self.program), moved)
        self.program = os.path.join(moved, "wc2bne-gui")
        self.run_program("--add-to-menu")
        with open(self.data(ENTRY), encoding="utf-8") as f:
            self.assertIn('Exec="%s"\n' % self.program, f.read())

    def test_removes_only_what_it_added(self):
        os.makedirs(self.data("applications"))
        with open(self.data("applications", "other.desktop"), "w") as f:
            f.write("[Desktop Entry]\n")
        self.run_program("--add-to-menu")
        code, output = self.run_program("--remove-from-menu")
        self.assertEqual(code, 0, output)
        self.assertFalse(os.path.exists(self.data(ENTRY)))
        self.assertFalse(os.path.exists(self.data(ICON)))
        self.assertTrue(os.path.exists(self.data("applications", "other.desktop")))
        self.assertEqual(self.run_program("--remove-from-menu")[0], 0)

    def test_help_version_and_unknown_options(self):
        code, output = self.run_program("--help")
        self.assertEqual(code, 0)
        self.assertIn("--add-to-menu", output)
        self.assertEqual(self.run_program("--version"), (0, "wc2bne-gui 0.1.0\n"))
        code, output = self.run_program("--fly")
        self.assertEqual(code, 2)
        self.assertIn("unknown option: --fly", output)
