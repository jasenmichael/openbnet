"""The window: shows the state and reports clicks."""
from gi.repository import Gdk, GLib, Gtk

from .servers import Servers
from .settings import Settings
from .launcher import can_run_as_root, run_as_root
from .state import MISSING_PROGRAM, Progress, explain, follow, missing_text, play_view, programs_problem

CHECK_EVERY = 2000  # milliseconds

EDITIONS = (("community", "Community", "for PvPGN servers"), ("official", "Official", "for Blizzard's server"))
ACTIONS = {"play": "Play", "install": "Install", "reinstall": "Reinstall"}
MODES = (("fullscreen", "Fullscreen", ""), ("window", "Window", "640x480"))


class Window(Gtk.ApplicationWindow):
    def __init__(self, app, launcher):
        super().__init__(application=app, title="Warcraft II on Linux")
        self.launcher = launcher
        self.connect("delete-event", self.on_close)
        self.status = None  # what the launcher last reported
        self.status_problem = ("", "")  # why it couldn't
        self.problems = {}  # edition -> what went wrong last, and the launcher's message
        self.progress = None  # of the install this window is doing
        self.job = None  # that install
        self.playing = set()  # editions whose game this window started
        self.checking = False  # true while status runs
        self.reported = {}  # the settings status last gave
        self.changes = 0  # choices made by the player
        self.to_save = []  # choices being saved, one at a time, as (setting, value)
        self.showing = False  # true while controls are set from the state, not by the player
        tabs = Gtk.Notebook()
        self.add(tabs)
        play = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12, margin=18)
        tabs.append_page(play, Gtk.Label(label="Play"))
        row = Gtk.Box(spacing=36)
        play.add(row)
        self.servers = Servers(launcher, self.ask)
        tabs.append_page(self.servers, Gtk.Label(label="Servers"))
        tabs.connect("switch-page", lambda tabs, page, number: page is self.servers and self.servers.load(self.edition()))
        self.settings = Settings(launcher, self.ask, self.refresh, self.mode,
                                 lambda: self.status is None and self.status_problem[0] == explain(2))
        self.settings.check_programs = self.check_programs
        tabs.append_page(self.settings, Gtk.Label(label="Settings"))
        self.editions = self.radios(row, "Edition", "edition", EDITIONS)
        self.modes = self.radios(row, "Display", "mode", MODES)
        self.play = Gtk.Button(label="Play")
        self.play.connect("clicked", self.on_action)
        self.play.get_style_context().add_class("suggested-action")
        self.play.set_size_request(-1, 56)
        play.add(self.play)
        self.bar = Gtk.ProgressBar()
        play.add(self.bar)
        row = Gtk.Box(spacing=12, homogeneous=True)
        play.pack_end(row, False, False, 0)
        self.open_game = Gtk.Button(label="Open game folder")
        self.open_game.connect("clicked", self.on_open, [])
        self.open_maps = Gtk.Button(label="Open maps")
        self.open_maps.connect("clicked", self.on_open, ["--maps"])
        row.add(self.open_game)
        row.add(self.open_maps)
        self.cancel = Gtk.Button(label="Cancel")
        self.cancel.connect("clicked", lambda button: self.job and self.job.interrupt())
        self.programs = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        play.add(self.programs)
        self.missing = Gtk.Label(wrap=True, max_width_chars=48, xalign=0)
        self.programs.add(self.missing)
        self.commands = Gtk.Label(wrap=True, max_width_chars=48, xalign=0, selectable=True)
        self.programs.add(self.commands)
        self.run_yourself = Gtk.Label(label="Run these in a terminal as root.", xalign=0)
        self.programs.add(self.run_yourself)
        self.install_programs = Gtk.Button(label="Install programs")
        self.install_programs.connect("clicked", self.on_install_programs)
        self.programs.add(self.install_programs)
        self.copy = Gtk.Button(label="Copy commands")
        self.copy.connect("clicked", lambda button: Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(
            "\n".join(self.needed_commands), -1))
        self.programs.add(self.copy)
        self.programs_message = Gtk.Label(wrap=True, max_width_chars=48, xalign=0, selectable=True)
        self.programs.add(self.programs_message)
        self.message = Gtk.Label(wrap=True, max_width_chars=48, xalign=0)
        row = Gtk.Box(spacing=12)
        row.pack_start(self.message, True, True, 0)
        row.pack_end(self.cancel, False, False, 0)
        play.add(row)
        self.details = Gtk.Label(wrap=True, max_width_chars=48, xalign=0, selectable=True)
        play.add(self.details)
        self.show_all()
        self.update()
        self.programs.hide()
        self.check_programs()
        self.refresh()
        GLib.timeout_add(CHECK_EVERY, self.refresh)
        GLib.timeout_add(150, self.pulse)

    def check_programs(self):
        self.launcher.doctor(self.show_programs)

    def show_programs(self, missing, commands, states=()):
        self.settings.show_programs(states)
        self.missing.set_text(missing_text(missing))
        self.needed_commands = commands
        self.commands.set_text("\n".join(commands))
        can_install = bool(commands) and can_run_as_root()
        self.copy.set_visible(bool(commands))
        self.commands.set_visible(bool(commands))
        self.install_programs.set_visible(can_install)
        self.run_yourself.set_visible(bool(commands) and not can_install)
        self.programs.set_visible(bool(missing))

    def on_install_programs(self, button):
        self.install_programs.set_sensitive(False)
        run_as_root(self.needed_commands, self.on_programs_installed)

    def on_programs_installed(self, result):
        def show(missing, commands, states):
            self.install_programs.set_sensitive(True)
            self.show_programs(missing, commands, states)
            problem = programs_problem(result.code, missing)
            if problem and result.error.strip():
                problem += "\n" + result.error.strip()
            self.programs_message.set_text(problem)
            self.programs_message.show()
        self.launcher.doctor(show)

    def pulse(self):
        """Keeps the bar moving during steps that don't say how far they are."""
        if self.progress and self.progress.percent is None:
            self.bar.pulse()
        return True  # keep the timer

    def refresh(self):
        if not self.checking:
            self.checking = True
            changes = self.changes
            self.launcher.status(lambda status, result: self.show_status(status, result, changes))
        return True  # keep the timer

    def radios(self, parent, title, setting, choices):
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        parent.add(column)
        heading = Gtk.Label(xalign=0)
        heading.set_markup("<b>%s</b>" % title)
        column.add(heading)
        buttons, first = {}, None
        for value, label, hint in choices:
            button = buttons[value] = Gtk.RadioButton.new_with_label_from_widget(first, label)
            first = first or button
            button.connect("toggled", self.on_choose, setting, value)
            row = Gtk.Box(spacing=6)
            row.add(button)
            note = Gtk.Label(label=hint)
            note.get_style_context().add_class("dim-label")
            row.add(note)
            column.add(row)
        return buttons

    def on_choose(self, button, setting, value):
        if button.get_active() and not self.showing:
            if setting == "edition" and self.servers.changed():
                previous = self.servers.edition

                def discard():
                    self.servers.show_list(self.servers.saved)
                    button.set_active(False)
                    button.set_active(True)

                self.showing = True
                self.editions[previous].set_active(True)
                self.showing = False
                self.ask("Discard unsaved server changes?", "They are for the %s edition." % previous.capitalize(),
                         "Keep editing", "Discard changes", discard)
                return
            self.update()
            self.changes += 1
            self.to_save.append((setting, value))
            if len(self.to_save) == 1:
                self.save_next()

    def save_next(self):
        setting, value = self.to_save[0]
        self.launcher.config_set(setting, value, self.on_saved)

    def edition(self):
        return next(e for e, button in self.editions.items() if button.get_active())

    def mode(self):
        return next(m for m, button in self.modes.items() if button.get_active())

    def on_action(self, button):
        if self.action == "install":
            self.start_install(self.edition())
        elif self.action == "reinstall":
            self.reinstall()
        else:
            self.start_game()

    def reinstall(self):
        edition = self.edition()
        self.ask("Reinstall this edition?", "Its game folder is deleted, with its saved games and settings. "
                 "Shared maps and the other edition stay.", "Keep it", "Delete and reinstall",
                 lambda: self.remove_then_install(edition))

    def remove_then_install(self, edition):
        self.progress = Progress("remove", None)
        self.launcher.uninstall(edition, lambda result: self.on_uninstalled(edition, result))
        self.update()

    def on_uninstalled(self, edition, result):
        if result.code == 0:
            self.start_install(edition)
        else:
            self.progress = None
            self.on_problem(edition, result)

    def start_install(self, edition):
        self.problems.pop(edition, None)
        self.progress = Progress("", None)
        self.job = self.launcher.install(edition, self.on_progress, lambda result: self.on_installed(edition, result))
        self.update()

    def on_close(self, window, event):
        if self.job is None and self.servers.changed():
            self.ask("Close without saving?", "Your changes to the servers will be lost.", "Keep editing",
                     "Close without saving", self.destroy)
            return True
        if self.job is None:
            return False
        self.ask("Stop installing?", "The install can be started again later.", "Keep installing", "Stop and close",
                 self.stop_and_close)
        return True

    def stop_and_close(self):
        if self.job:
            self.job.interrupt()
        self.destroy()

    def ask(self, title, text, no, yes, then):
        """Calls then() if the player chooses yes. Doesn't wait, because waiting would block screen readers."""
        dialog = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.QUESTION, text=title,
                                   secondary_text=text)
        dialog.add_button(no, Gtk.ResponseType.CANCEL)
        dialog.add_button(yes, Gtk.ResponseType.OK)
        dialog.set_default_response(Gtk.ResponseType.CANCEL)

        def answered(dialog, answer):
            dialog.destroy()
            if answer == Gtk.ResponseType.OK:
                then()
        dialog.connect("response", answered)
        dialog.present()

    def on_progress(self, line):
        self.progress = follow(self.progress, line)
        self.update()

    def on_installed(self, edition, result):
        self.job = None

        def show(status, answer):
            self.progress = None
            self.show_status(status, answer, self.changes)
            self.on_problem(edition, result, "install")
        self.launcher.status(show)

    def start_game(self):
        edition = self.edition()
        self.playing.add(edition)
        self.problems.pop(edition, None)
        self.launcher.play(edition, self.mode(), lambda result: self.on_played(edition, result))
        self.update()

    def on_played(self, edition, result):
        self.playing.discard(edition)
        self.on_problem(edition, result)

    def update(self):
        """Shows the state."""
        problem, details = self.problems.get(self.edition(), ("", ""))
        if self.status is None:
            problem, details = self.status_problem
        view = play_view(self.status, self.edition(), self.playing, problem, self.progress)
        self.details.set_text(details)
        self.details.set_visible(bool(details))
        self.message.set_text(view.message)
        self.cancel.set_visible(self.job is not None)
        self.bar.set_visible(self.progress is not None)
        if self.progress and self.progress.percent is not None:
            self.bar.set_fraction(min(self.progress.percent, 100) / 100)
        self.play.set_label(ACTIONS[view.action])
        self.play.set_sensitive(view.play)
        self.action = view.action
        self.servers.show_state(self.edition(), self.status)
        self.settings.show_state(self.edition(), self.status)
        self.open_game.set_sensitive(view.open)
        self.open_maps.set_sensitive(view.open)
        for button in self.editions.values():
            button.set_sensitive(view.edition)
        for button in self.modes.values():
            button.set_sensitive(view.mode)

    def on_open(self, button, options):
        edition = self.edition()
        self.launcher.open(edition, options, lambda result: self.on_problem(edition, result))

    def on_problem(self, edition, result, command=""):
        if result.code != 0:
            self.problems[edition] = (explain(result.code, command), result.error.strip())
        if result.code == MISSING_PROGRAM:
            self.check_programs()
        self.update()

    def on_saved(self, result):
        self.to_save.pop(0)
        self.on_problem(self.edition(), result)
        if self.to_save:
            self.save_next()

    def show_status(self, status, result, changes):
        """changes: self.changes when status started; an answer from before a choice is out of date."""
        self.checking = False
        self.status = status
        self.servers.refresh()
        self.status_problem = ("", "") if status else (explain(result.code), result.error.strip())
        if status and changes == self.changes and not self.to_save:
            self.showing = True
            for setting, buttons in (("edition", self.editions), ("mode", self.modes)):
                if self.reported.get(setting) != status[setting]:
                    self.reported[setting] = status[setting]
                    buttons[status[setting]].set_active(True)
            self.showing = False
        self.update()
