"""Shared test setup: a private bus and hidden display, a stand-in wc2bne, and the real window driven through the
accessibility interface (what a screen reader sees). Nothing here may reach the real desktop, launcher or data folder."""
import ctypes
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.dirname(HERE)
WAIT = 10
WINDOW_TITLE = "Warcraft II on Linux"


def enter_private_session():
    """Run this process again with its own bus and display, unless that has been done."""
    if os.environ.get("WC2BNE_GUI_TEST"):
        return
    home = tempfile.mkdtemp(prefix="wc2bne-gui-test-")
    run = os.path.join(home, "run")
    os.mkdir(run, 0o700)
    env = {"PATH": os.environ["PATH"], "HOME": home, "XDG_RUNTIME_DIR": run, "LANG": "C.UTF-8", "TMPDIR": home,
           "GDK_BACKEND": "x11", "GSETTINGS_BACKEND": "memory", "GTK_USE_PORTAL": "0", "ATSPI_DBUS_IMPLEMENTATION": "dbus-daemon",
           "WC2BNE_GUI_TEST": home}
    services = os.path.join(home, "services")
    os.mkdir(services)
    shutil.copy("/usr/share/dbus-1/services/org.a11y.Bus.service", services)
    with open(os.path.join(HERE, "bus.conf")) as f:
        config = f.read().replace("SERVICES", services)
    with open(os.path.join(home, "bus.conf"), "w") as f:
        f.write(config)
    command = ["xvfb-run", "-a", "-s", "-screen 0 1024x768x24", "dbus-run-session",
               "--config-file=" + os.path.join(home, "bus.conf"), "--", sys.executable] + sys.argv
    try:
        code = subprocess.call(command, env=env)
    finally:
        shutil.rmtree(home, ignore_errors=True)
    sys.exit(code)


def wait_for(check, what):
    end = time.monotonic() + WAIT
    while True:
        value = check()
        if value:
            return value
        if time.monotonic() > end:
            raise AssertionError("timed out waiting for " + (what() if callable(what) else what))
        time.sleep(0.02)


def close_window(title):
    """Asks the top-level window with this title to close, as a window manager would (there is none here)."""
    x = ctypes.CDLL("libX11.so.6")
    ID = ctypes.c_ulong
    x.XOpenDisplay.restype = ctypes.c_void_p
    x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x.XDefaultRootWindow.restype = ID
    x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    x.XInternAtom.restype = ID
    x.XQueryTree.argtypes = [ctypes.c_void_p, ID] + [ctypes.c_void_p] * 4
    x.XFetchName.argtypes = [ctypes.c_void_p, ID, ctypes.POINTER(ctypes.c_char_p)]
    x.XSendEvent.argtypes = [ctypes.c_void_p, ID, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
    x.XFree.argtypes = [ctypes.c_void_p]
    x.XCloseDisplay.argtypes = [ctypes.c_void_p]

    class Message(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ID), ("send_event", ctypes.c_int),
                    ("display", ctypes.c_void_p), ("window", ID), ("message_type", ID), ("format", ctypes.c_int),
                    ("data", ctypes.c_long * 5), ("padding", ctypes.c_long * 24)]

    display = x.XOpenDisplay(None)
    root, parent, count = ID(), ID(), ctypes.c_uint()
    children = ctypes.POINTER(ID)()
    x.XQueryTree(display, x.XDefaultRootWindow(display), ctypes.byref(root), ctypes.byref(parent),
                 ctypes.byref(children), ctypes.byref(count))
    found = False
    for i in range(count.value):
        name = ctypes.c_char_p()
        if x.XFetchName(display, children[i], ctypes.byref(name)) and name.value == title.encode():
            event = Message(type=33, window=children[i], format=32)
            event.message_type = x.XInternAtom(display, b"WM_PROTOCOLS", 0)
            event.data[0] = x.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
            x.XSendEvent(display, children[i], 0, 0, ctypes.byref(event))
            found = True
    x.XCloseDisplay(display)  # also sends what is queued
    assert found, "no window called " + title


class WindowTest(unittest.TestCase):
    """A test of the real window. self.launcher scripts the stand-in; self.start() opens the window."""

    def setUp(self):
        self.dir = tempfile.mkdtemp(dir=os.environ["WC2BNE_GUI_TEST"])
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.home = self.path("home")
        self.bin = self.path("bin")
        os.mkdir(self.home)
        os.mkdir(self.bin)
        os.mkdir(self.path("answers"))
        # A copy, so that the real launcher beside this project can't be found.
        self.program = self.path("repo", "wc2bne-gui", "wc2bne-gui")
        shutil.copytree(PROJECT, os.path.dirname(self.program),
                        ignore=shutil.ignore_patterns("test", "__pycache__"))
        self.launcher = self.stand_in(self.path("wc2bne"))
        self.env = dict(os.environ, HOME=self.home, WC2BNE=self.launcher, STUB=self.dir, PATH=self.bin)
        for name in ("python3", "env", "sh", "bash"):
            os.symlink(shutil.which(name), os.path.join(self.bin, name))
        self.answer("status", STATUS)
        self.answer("doctor", DOCTOR)
        self.processes = []
        self.addCleanup(self.stop)

    def path(self, *names):
        return os.path.join(self.dir, *names)

    def stand_in(self, path):
        shutil.copy(os.path.join(HERE, "stand-in"), path)
        os.chmod(path, 0o755)
        return path

    def answer(self, command, output="", code=0, error="", hold=False):
        """What the stand-in does for a command, such as "status" or "config set". With hold, until release()."""
        base = self.path("answers", command.replace(" ", "-"))
        self.release(command)
        for name, text in (("out", output), ("err", error), ("code", str(code))):
            with open(base + "." + name + ".new", "w", encoding="utf-8") as f:
                f.write(text)
            os.rename(base + "." + name + ".new", base + "." + name)
        if hold:
            open(base + ".hold", "w").close()

    def more(self, command, output):
        """More output from a command that is being held."""
        path = self.path("answers", command.replace(" ", "-") + ".more")
        with open(path + ".new", "w", encoding="utf-8") as f:
            f.write(output)
        os.rename(path + ".new", path)
        wait_for(lambda: not os.path.exists(path), "the stand-in to print " + output)

    def release(self, command):
        try:
            os.remove(self.path("answers", command.replace(" ", "-") + ".hold"))
        except FileNotFoundError:
            pass

    def status(self, **changes):
        """Change what status reports."""
        lines = dict(line.split("=", 1) for line in STATUS.splitlines())
        lines.update(changes)
        self.answer("status", "".join("%s=%s\n" % pair for pair in lines.items()))

    def start(self, *args, before=()):
        process = subprocess.Popen(list(before) + [self.program] + list(args), env=self.env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, universal_newlines=True)
        self.processes.append(process)
        return process

    def open(self):
        self.start()
        self.window()

    def stop(self):
        for command in os.listdir(self.path("answers")):
            if command.endswith(".hold"):
                os.remove(self.path("answers", command))
        for process in self.processes:
            if process.poll() is None:
                process.kill()
            output = process.communicate()[0]
            bad = [line for line in output.splitlines()
                   if "Traceback" in line or "CRITICAL" in line or "Gtk-WARNING" in line]
            self.assertEqual(bad, [], output)

    def ran(self):
        """The commands run so far that aren't just reads, as lists of arguments."""
        try:
            with open(self.path("ran"), encoding="utf-8") as f:
                lines = f.read().splitlines()
        except FileNotFoundError:
            return []
        commands = [line.split("\x1f") for line in lines]
        return [c for c in commands if c[0] not in ("status", "doctor", "--version") and c[:2] != ["gateway", "list"]
                and c[:2] not in (["config", "list"], ["config", "get"])]

    def wait_checks(self, count=2):
        """Wait until the window has asked for the status count more times."""
        def checks():
            with open(self.path("ran"), encoding="utf-8") as f:
                return sum(line.startswith("status") for line in f)
        before = checks()
        wait_for(lambda: checks() >= before + count, "another look at the status")

    def all_ran(self):
        """Every command run so far, reads too."""
        try:
            with open(self.path("ran"), encoding="utf-8") as f:
                return [line.split("\x1f") for line in f.read().splitlines()]
        except FileNotFoundError:
            return []

    def wait_ran(self, *command):
        wait_for(lambda: list(command) in self.ran(), lambda: "command %r; ran %r" % (command, self.ran()))

    # What a player sees and does.

    def window(self):
        return wait_for(self.find_window, "the window")

    def apps(self):
        from gi.repository import Atspi
        pids = [p.pid for p in self.processes if p.poll() is None]
        desktop = Atspi.get_desktop(0)
        found = []
        for i in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(i)
            if app is not None and app.get_process_id() in pids:
                found.append(app)
        return found

    def find_window(self):
        for app in self.apps():
            if app.get_child_count():
                return app.get_child_at_index(0)
        return None

    def everything(self):
        """Every showing control, as (role, name, object)."""
        from gi.repository import Atspi, GLib
        found = []

        def walk(node):
            try:
                if not node.get_state_set().contains(Atspi.StateType.SHOWING):
                    return
                text = node.get_name() or ""
                role = node.get_role_name()
                if role == "label" or (role == "text" and not text):
                    content = Atspi.Text.get_text(node, 0, -1) if node.get_text_iface() else ""
                    text = content or text
                found.append((role, text, node))
                for i in range(node.get_child_count()):
                    child = node.get_child_at_index(i)
                    if child is not None:
                        walk(child)
            except GLib.Error:
                pass  # It went away while we looked.

        for app in self.apps():
            app.clear_cache()
            for i in range(app.get_child_count()):
                window = app.get_child_at_index(i)
                if window is not None:
                    walk(window)
        return found

    def control(self, name, dialog=False):
        """The showing control with this name; with dialog, only one in a dialog."""
        found = [node for role, text, node in self.everything() if text == name and role not in ("label", "page tab")]
        if dialog:
            found = [node for node in found if self.in_dialog(node)]
        return found[-1] if found else None

    def in_dialog(self, node):
        while node is not None:
            if node.get_role_name() in ("dialog", "alert"):
                return True
            node = node.get_parent()
        return False

    def wait_control(self, name):
        return wait_for(lambda: self.control(name), lambda: "control %r; showing %r" % (name, self.shown()))

    def shown(self):
        return [text for _, text, _ in self.everything() if text]

    def has(self, node, state):
        from gi.repository import Atspi
        return node.get_state_set().contains(getattr(Atspi.StateType, state))

    def enabled(self, name):
        node = self.control(name)
        return node is not None and self.has(node, "SENSITIVE")

    def chosen(self, name):
        node = self.control(name)
        return node is not None and self.has(node, "CHECKED")

    def wait_enabled(self, name):
        wait_for(lambda: self.enabled(name), lambda: "%r to be enabled; showing %r" % (name, self.shown()))

    def wait_disabled(self, name):
        wait_for(lambda: self.control(name) and not self.enabled(name), "%r to be disabled" % name)

    def wait_chosen(self, name):
        wait_for(lambda: self.chosen(name), "%r to be chosen" % name)

    def wait_gone(self, name):
        wait_for(lambda: self.control(name) is None, "%r to go away" % name)

    def wait_text(self, text):
        wait_for(lambda: any(text in shown for shown in self.shown()), lambda: "text %r; showing %r" % (text, self.shown()))

    def close(self):
        """Closes the window like its X button does."""
        self.window()
        close_window(WINDOW_TITLE)

    def clipboard(self):
        code = ("import gi; gi.require_version('Gtk', '3.0'); from gi.repository import Gdk, Gtk; "
                "print(Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).wait_for_text(), end='')")
        return subprocess.check_output([sys.executable, "-c", code], universal_newlines=True)

    def tab(self, name):
        """Shows a tab of the window."""
        for role, text, node in self.everything():
            if role == "page tab list":
                selection = node.get_selection_iface()
                for i in range(node.get_child_count()):
                    if node.get_child_at_index(i).get_name() == name:
                        selection.select_child(i)
                        return
        raise AssertionError("no tab " + name)

    def text_of(self, name):
        node = self.control(name)
        from gi.repository import Atspi
        return node and node.get_text_iface() and Atspi.Text.get_text(node, 0, -1)

    def type(self, name, text):
        """Replaces the text in a field."""
        self.wait_enabled(name)
        self.control(name).get_editable_text_iface().set_text_contents(text)

    def click(self, name, dialog=False):
        wait_for(lambda: self.control(name, dialog) and self.has(self.control(name, dialog), "SENSITIVE"),
                 lambda: "%r to be enabled; showing %r" % (name, self.shown()))
        self.control(name, dialog).get_action_iface().do_action(0)


STATUS = """\
data=/games/war2
installer=present
edition=community
mode=fullscreen
community=installed
official=absent
community_in_use=no
official_in_use=no
"""

DOCTOR = """\
bwrap=ok
wine=ok
wine32=ok
curl=ok
xrandr=ok
xdg-open=ok
Xwayland=ok
"""
