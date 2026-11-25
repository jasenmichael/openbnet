"""The Settings tab: data folder, UDP port, required programs, uninstalling, and versions."""
import os

from gi.repository import Gtk

from . import VERSION
from .state import explain, program_line


class Settings(Gtk.Box):
    def __init__(self, launcher, ask, changed, mode, broken):
        """mode(): the chosen display mode. broken(): whether the settings file is invalid."""
        """ask: the window's way of asking the player. changed(): call when the launcher's state may have changed."""
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=12, margin=18)
        self.launcher = launcher
        self.ask = ask
        self.changed = changed
        self.mode = mode
        self.broken = broken
        self.edition = None
        self.repair = Gtk.Button(label="Repair settings", halign=Gtk.Align.START)
        self.repair.connect("clicked", self.on_repair)
        self.add(self.repair)
        self.add(heading("Data folder"))
        row = Gtk.Box(spacing=6)
        self.add(row)
        self.folder = Gtk.Entry(hexpand=True)
        self.folder.get_accessible().set_name("Data folder")
        row.add(self.folder)
        self.use_folder = Gtk.Button(label="Use this folder")
        self.use_folder.connect("clicked", self.on_use_folder)
        row.add(self.use_folder)
        self.default_folder = Gtk.Button(label="Use the default folder", halign=Gtk.Align.START)
        self.default_folder.connect("clicked", lambda button: self.launcher.config_set(
            "data-folder", "default", lambda result: self.after_folder(result)))
        self.add(self.default_folder)
        self.add(note("Nothing is moved: the game uses what is in the folder you choose."))
        if os.environ.get("WC2BNE_HOME"):
            self.add(note("The WC2BNE_HOME environment variable decides it, so it can't be changed here."))
            for widget in (self.folder, self.use_folder, self.default_folder):
                widget.set_sensitive(False)
        self.add(heading("Network"))
        row = Gtk.Box(spacing=6)
        self.add(row)
        title = Gtk.Label(label="UDP port")
        row.add(title)
        self.port = Gtk.SpinButton.new_with_range(1024, 65435, 1)
        self.port.get_accessible().set_name("UDP port")
        title.set_mnemonic_widget(self.port)
        row.add(self.port)
        self.save_port = Gtk.Button(label="Save port")
        self.save_port.connect("clicked", self.on_save_port)
        row.add(self.save_port)
        self.port_message = Gtk.Label(xalign=0, wrap=True, max_width_chars=48, selectable=True)
        self.add(self.port_message)
        self.add(note("The port the game uses for matches. Forward it on your router to host games."))
        self.add(heading("Required programs"))
        self.programs = Gtk.Label(xalign=0, selectable=True)
        self.add(self.programs)
        self.check = Gtk.Button(label="Check again", halign=Gtk.Align.START)
        self.check.connect("clicked", lambda button: self.check_programs())
        self.add(self.check)
        self.check_programs = lambda: None  # set by the window
        self.add(heading("Uninstall"))
        self.uninstall = Gtk.Button(label="Uninstall", halign=Gtk.Align.START)
        self.uninstall.connect("clicked", self.on_uninstall)
        self.add(self.uninstall)
        self.uninstall_message = Gtk.Label(xalign=0, wrap=True, max_width_chars=48, selectable=True)
        self.add(self.uninstall_message)
        self.about = note("wc2bne-gui %s. Free software under the GPL version 3 or later. Not connected with Blizzard "
                          "Entertainment." % VERSION)
        self.add(self.about)
        self.launcher.version(lambda text: text and self.about.set_text(self.about.get_text().replace(
            VERSION + ".", "%s, %s." % (VERSION, text), 1)))
        self.connect("map", lambda widget: (self.load(), self.repair.set_visible(self.broken())))

    def show_programs(self, states):
        self.programs.set_text("\n".join(program_line(name, state) for name, state in states))

    def show_state(self, edition, status):
        """The chosen edition, and what the launcher reported, or None."""
        free = bool(status) and status[edition] != "absent" and status[edition + "_in_use"] == "no"
        for widget in (self.port, self.save_port, self.uninstall):
            widget.set_sensitive(free)
        self.repair.set_visible(status is None and self.broken())
        self.uninstall.set_label("Uninstall the %s edition" % edition.capitalize())
        if edition != self.edition:
            self.edition = edition
            if self.get_mapped():
                self.load()

    def on_repair(self, button):
        """Writes the edition and mode again, which replaces invalid ones."""
        self.launcher.config_set("edition", self.edition, lambda result: self.launcher.config_set(
            "mode", self.mode(), lambda result: self.changed()))

    def on_uninstall(self, button):
        edition = self.edition
        self.ask("Uninstall the %s edition?" % edition.capitalize(),
                 "Its game folder is deleted, with its saved games and settings. Shared maps, the installer and the "
                 "other edition stay.", "Keep it", "Uninstall", lambda: self.launcher.uninstall(edition, self.uninstalled))

    def uninstalled(self, result):
        self.uninstall_message.set_text("" if result.code == 0 else explain(result.code) + "\n" + result.error.strip())
        self.changed()

    def on_save_port(self, button):
        self.port.update()
        self.port_message.set_text("")
        self.launcher.config_set_for(self.edition, "port", str(self.port.get_value_as_int()), self.on_port_saved)

    def on_port_saved(self, result):
        if result.code == 0:
            self.port_message.set_text("Saved.")
        else:
            self.port_message.set_text(explain(result.code) + " The port must be 1024 to 65435.\n" + result.error.strip())

    def load(self):
        self.launcher.config_get("port", self.edition, lambda value, result: value and self.port.set_value(int(value)))
        self.launcher.config_get("data-folder", None, lambda value, result: self.folder.set_text(value or ""))

    def on_use_folder(self, button):
        self.launcher.config_set("data-folder", self.folder.get_text().strip(), self.after_folder)

    def after_folder(self, result):
        self.load()
        self.changed()


def heading(text):
    label = Gtk.Label(xalign=0)
    label.set_markup("<b>%s</b>" % text)
    return label


def note(text):
    label = Gtk.Label(label=text, xalign=0, wrap=True, max_width_chars=48)
    label.get_style_context().add_class("dim-label")
    return label
