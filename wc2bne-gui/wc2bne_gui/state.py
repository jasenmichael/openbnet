"""What the window shows and allows, worked out from what the launcher reported. Everything here is pure."""
import re
from collections import namedtuple

View = namedtuple("View", "message action play mode open edition")

MISSING_PROGRAM = 4

EXPLANATIONS = {
    1: "It didn't work.",
    2: "A setting isn't valid.",
    3: "This edition isn't installed.",
    4: "A program it needs is missing.",
    5: "A game file has changed, so servers would refuse it.",
    6: "This edition is in use.",
}


INTERRUPTED = (130, -2)  # it said so, or the interrupt ended it


def explain(code, command=""):
    """An exit code of the launcher's command -> what it means."""
    if command == "install" and code == 5:
        return "No installer with the right checksum was found or could be downloaded, so it wasn't run."
    if code in INTERRUPTED:
        return "Cancelled."
    return EXPLANATIONS.get(code, "It stopped unexpectedly (%d)." % code)


STEPS = {"download": "Downloading the installer", "prepare": "Preparing Wine", "install": "Installing the game",
         "remove": "Removing the old files", "finish": "Finishing", "done": "Finishing"}

# An install in progress: its step, and how much of it is done in percent, or None.
Progress = namedtuple("Progress", "step percent")


def follow(progress, line):
    """The progress after a line of the launcher's output."""
    name, _, value = line.partition("=")
    if name == "step":
        return Progress(value, None)
    if name == "percent" and value.isdigit():
        return Progress(progress.step, int(value))
    return progress


def describe(progress):
    text = STEPS.get(progress.step, "Installing")
    return text if progress.percent is None else "%s: %d%%" % (text, progress.percent)


PROGRAMS = {"bwrap": "bubblewrap", "wine": "Wine", "wine32": "Wine's 32-bit libraries"}


def programs_problem(code, missing):
    """How installing programs ended, and what is still missing -> what to say, or nothing."""
    if not missing:
        return ""
    if code in (126, 127):
        return "Permission wasn't given, so nothing was installed."
    return "Some programs are still missing."


def program_line(name, state):
    """-> how one program is shown in Settings."""
    text = {"ok": "installed", "missing": "not installed", "optional": "not installed (optional)"}.get(state, state)
    return "%s: %s" % (PROGRAMS.get(name, name), text)


def missing_text(missing):
    """Names of missing programs, as doctor gives them -> what to say."""
    return "Missing: " + ", ".join(PROGRAMS.get(name, name) for name in missing)


# A gateway: whether it is selected, its host, GMT offset and name.
Gateway = namedtuple("Gateway", "selected host offset name")


def gateways(text):
    """gateway list --porcelain -> a list of Gateway."""
    found = []
    for line in text.splitlines():
        fields = line.split("\t")
        if len(fields) == 5:
            found.append(Gateway(fields[1] == "1", fields[2], fields[3], fields[4]))
    return found


def gateway_problem(host, name):
    """-> why a new gateway can't be added, or nothing. The launcher's rules, checked early."""
    if host and not re.fullmatch(r"[A-Za-z0-9.-]+", host):
        return "An address may only have letters, digits, dots and dashes."
    if "|" in name or "\\" in name:
        return "A name can't contain | or \\."
    if not host or not name.strip():
        return "Give an address and a name."
    return ""


def rows(found):
    """A list of Gateway -> what gateway set reads."""
    return "".join("%d\t%d\t%s\t%s\t%s\n" % (n, g.selected, g.host, g.offset, g.name) for n, g in enumerate(found, 1))


def select(found, number):
    """-> the list with only the gateway at that index selected."""
    return [g._replace(selected=i == number) for i, g in enumerate(found)]


def remove(found, number):
    """-> the list without the gateway at that index; the selected one stays, or the first is selected."""
    rest = found[:number] + found[number + 1:]
    if found[number].selected and rest:
        rest = select(rest, 0)
    return rest


def play_view(status, edition, playing, problem, progress):
    """status: what the launcher reports, or None before the first answer. playing: editions this window started.
    problem: what went wrong last, or empty. progress: of the install this window is doing, or None."""
    if progress:
        return View(describe(progress), "install", False, False, False, False)
    if status is None:
        return View(problem, "play", False, False, False, True)
    if edition in playing or status[edition + "_in_use"] == "play":
        return View("Running", "play", False, False, True, True)
    if status[edition + "_in_use"] != "no":
        return View("In use by: wc2bne " + status[edition + "_in_use"], "play", False, False, True, True)
    if status[edition] == "absent":
        return View(problem or explain(3), "install", True, True, False, True)
    if status[edition] == "changed":
        return View(problem or explain(5), "reinstall", True, True, True, True)
    return View(problem or "Ready", "play", True, True, True, True)
