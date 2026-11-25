"""The only part that runs commands: wc2bne's command line, as its specification describes it."""
import os
import shutil
import signal
import subprocess
import tempfile
import threading

from .state import gateways, rows

from gi.repository import GLib


def pairs(text):
    """NAME=VALUE lines -> dict. Pure."""
    return dict(line.split("=", 1) for line in text.splitlines() if "=" in line)


class Job:
    """A command whose output is followed while it runs."""

    def __init__(self, command, on_line, done):
        """Calls on_line(text) for each line of output, then done(Result), in the window's thread."""
        self.errors = tempfile.TemporaryFile()
        self.process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                        stderr=self.errors, start_new_session=True)
        threading.Thread(target=self.follow, args=(on_line, done), daemon=True).start()

    def interrupt(self):
        """Stops it and what it started, like Ctrl+C."""
        if self.process.poll() is None:
            os.killpg(self.process.pid, signal.SIGINT)

    def follow(self, on_line, done):
        for line in self.process.stdout:
            GLib.idle_add(lambda text=line.decode("utf-8", "replace").rstrip("\n"): on_line(text) and False)
        code = self.process.wait()
        self.process.stdout.close()
        self.errors.seek(0)
        result = Result(code, "", self.errors.read().decode("utf-8", "replace"))
        self.errors.close()
        GLib.idle_add(lambda: done(result) and False)


def find_launcher(env, project):
    """-> the path of wc2bne. Reads files."""
    if env.get("WC2BNE"):
        return shutil.which(os.path.abspath(env["WC2BNE"]))
    sibling = os.path.join(os.path.dirname(project), "wc2bne-linux", "wc2bne")
    if os.access(sibling, os.X_OK):
        return sibling
    return shutil.which("wc2bne", path=env.get("PATH"))


def doctor(text):
    """What doctor reports -> names of needed programs that are missing, the commands that install them, and
    (name, ok|missing|optional) for every program. Pure."""
    missing, commands, states = [], [], []
    for line in text.splitlines():
        name, _, value = line.partition("=")
        if name == "install":
            commands.append(value)
        elif value:
            states.append((name, value))
            if value == "missing":
                missing.append(name)
    return missing, commands, states


class Result:
    """How a command ended: its exit code, output and error text."""

    def __init__(self, code, output, error):
        self.code = code
        self.output = output
        self.error = error


def run(command, done, given=None):
    """Runs command without blocking, with given as its input, then calls done(Result) in the window's thread.

    The command gets its own session and writes to files, so it isn't disturbed when the window closes."""
    def work():
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err, tempfile.TemporaryFile() as given_file:
            if given is not None:
                given_file.write(given.encode("utf-8"))
                given_file.seek(0)
            try:
                code = subprocess.Popen(command, stdin=given_file if given is not None else subprocess.DEVNULL, stdout=out, stderr=err,
                                        start_new_session=True).wait()
                out.seek(0)
                err.seek(0)
                result = Result(code, out.read().decode("utf-8", "replace"), err.read().decode("utf-8", "replace"))
            except OSError as error:
                result = Result(127, "", str(error))
        GLib.idle_add(lambda: done(result) and False)

    threading.Thread(target=work, daemon=True).start()


def can_run_as_root():
    return shutil.which("pkexec") is not None


def run_as_root(commands, done):
    """Runs shell commands in order as root, stopping at the first that fails. Asks for permission once."""
    run(["pkexec", "/bin/sh", "-c", "\n".join(["set -e"] + commands)], done)


class Launcher:
    def __init__(self, path):
        self.path = path

    def status(self, done):
        """done(dict of what status reports, or None, Result)"""
        run([self.path, "status", "--porcelain"], lambda r: done(pairs(r.output) if r.code == 0 else None, r))

    def config_set(self, name, value, done):
        run([self.path, "config", "set", name, value], done)

    def play(self, edition, mode, done):
        run([self.path, "play", "--edition", edition, "--" + mode], done)

    def open(self, edition, options, done):
        run([self.path, "open", "--edition", edition] + options, done)

    def install(self, edition, on_line, done):
        return Job([self.path, "install", "--edition", edition, "--porcelain"], on_line, done)

    def uninstall(self, edition, done):
        run([self.path, "uninstall", "--edition", edition, "--yes"], done)

    def doctor(self, done):
        """done(missing program names, commands that install them, (name, state) for each)"""
        run([self.path, "doctor", "--porcelain"], lambda r: done(*doctor(r.output)))

    def gateways(self, edition, done):
        """done(list of Gateway, or None, Result)"""
        run([self.path, "gateway", "list", "--porcelain", "--edition", edition],
            lambda r: done(gateways(r.output) if r.code == 0 else None, r))

    def set_gateways(self, edition, found, done):
        run([self.path, "gateway", "set", "--edition", edition], done, rows(found))

    def reset_gateways(self, edition, done):
        run([self.path, "gateway", "reset", "--edition", edition], done)

    def config_get(self, name, edition, done):
        """done(value or None, Result)"""
        command = [self.path, "config", "get", name] + (["--edition", edition] if edition else [])
        run(command, lambda r: done(r.output.strip() if r.code == 0 else None, r))

    def config_set_for(self, edition, name, value, done):
        run([self.path, "config", "set", name, value, "--edition", edition], done)

    def version(self, done):
        """done(text such as "wc2bne 0.1.0", or None)"""
        run([self.path, "--version"], lambda r: done(r.output.strip() if r.code == 0 else None))
