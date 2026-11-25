import os
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk  # noqa: E402

from .launcher import Launcher, find_launcher  # noqa: E402
from .window import Window  # noqa: E402
from . import VERSION, menu  # noqa: E402

PROJECT = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))


def explain(title, text):
    dialog = Gtk.MessageDialog(message_type=Gtk.MessageType.ERROR, buttons=Gtk.ButtonsType.CLOSE, text=title,
                               secondary_text=text, title="Warcraft II on Linux")
    dialog.run()
    dialog.destroy()


HELP = """Usage: wc2bne-gui [OPTION]
A window for installing and playing Warcraft II: Battle.net Edition with wc2bne.

  --add-to-menu       add it to the desktop's menu
  --remove-from-menu  remove it from the menu
  --help, --version

It uses the wc2bne named by WC2BNE, else the one in wc2bne-linux beside it, else the one on PATH."""


def main(argv):
    if argv[1:] in (["--help"], ["-h"]):
        print(HELP)
        return 0
    if argv[1:] == ["--version"]:
        print("wc2bne-gui " + VERSION)
        return 0
    if argv[1:] and argv[1:] not in (["--add-to-menu"], ["--remove-from-menu"]):
        print("wc2bne-gui: unknown option: %s (see: wc2bne-gui --help)" % " ".join(argv[1:]), file=sys.stderr)
        return 2
    if argv[1:] == ["--add-to-menu"]:
        menu.add(os.environ, os.path.join(PROJECT, "wc2bne-gui"))
        print("added to the menu")
        return 0
    if argv[1:] == ["--remove-from-menu"]:
        menu.remove(os.environ)
        print("removed from the menu")
        return 0
    path = find_launcher(os.environ, PROJECT)
    if os.environ.get("WC2BNE") and not path:
        explain("wc2bne wasn't found", "WC2BNE is set to " + os.environ["WC2BNE"] + ", which can't be run.")
        return 1
    if not path:
        explain("wc2bne wasn't found", "This window needs wc2bne-linux. Put it beside wc2bne-gui, put wc2bne on your "
                "PATH, or set WC2BNE to where it is.")
        return 1
    app = Gtk.Application(application_id="io.github.jasenmichael.wc2bne")
    app.connect("activate", lambda app: (app.get_active_window() or Window(app, Launcher(path))).present())
    return app.run(argv[:1])
