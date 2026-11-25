"""The Servers tab: the gateway list of the chosen edition, edited in the window and saved at once."""
from gi.repository import Gtk

from .state import Gateway, explain, gateway_problem, remove, select


class Servers(Gtk.Box):
    def __init__(self, launcher, ask):
        """ask: the window's way of asking the player, ask(title, text, no, yes, then)."""
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=12, margin=18)
        self.launcher = launcher
        self.ask = ask
        self.edition = None
        self.list = []  # Gateway, as shown
        self.saved = []  # Gateway, as the launcher has them
        self.usable = True  # the edition is free to change
        self.saving = False  # a save or reset is running
        self.title = Gtk.Label(xalign=0)
        self.add(self.title)
        self.absent = Gtk.Label(xalign=0)
        self.add(self.absent)
        self.editor = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self.add(self.editor)
        trust = Gtk.Label(label="Add only servers you trust: a server can run code inside the game's sandbox.",
                          xalign=0, wrap=True, max_width_chars=48)
        trust.get_style_context().add_class("dim-label")
        self.editor.add(trust)
        self.busy = Gtk.Label(label="Changes can be made when the game has closed.", xalign=0)
        self.editor.add(self.busy)
        self.rows = Gtk.ListBox(selection_mode=Gtk.SelectionMode.NONE)
        scroller = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, max_content_height=220,
                                      propagate_natural_height=True, shadow_type=Gtk.ShadowType.IN)
        scroller.add(self.rows)
        self.editor.add(scroller)
        self.connect("map", lambda widget: self.after_map())
        form = self.form = Gtk.Grid(column_spacing=12, row_spacing=6)
        self.editor.add(form)
        self.host = self.field(form, 0, "Address", Gtk.Entry(placeholder_text="server.example.com", hexpand=True))
        self.name = self.field(form, 1, "Name", Gtk.Entry())
        self.offset = self.field(form, 2, "GMT offset", Gtk.SpinButton.new_with_range(-12, 14, 1))
        self.offset.set_halign(Gtk.Align.START)
        self.offset.set_value(0)  # it starts at the lowest
        self.add_button = Gtk.Button(label="Add")
        self.add_button.connect("clicked", self.on_add)
        form.attach(self.add_button, 2, 2, 1, 1)
        self.form_problem = Gtk.Label(xalign=0, wrap=True, max_width_chars=48)
        self.editor.add(self.form_problem)
        for entry in (self.host, self.name):
            entry.connect("changed", lambda entry: self.check_form())
        self.check_form()
        buttons = Gtk.Box(spacing=12)
        self.editor.add(buttons)
        self.problem = Gtk.Label(xalign=0, wrap=True, max_width_chars=48, selectable=True)
        self.editor.add(self.problem)
        self.unsaved = Gtk.Label(label="Unsaved changes", xalign=0)
        buttons.pack_start(self.unsaved, True, True, 0)
        self.discard = Gtk.Button(label="Discard changes")
        self.discard.connect("clicked", lambda button: self.show_list(self.saved))
        buttons.pack_end(self.discard, False, False, 0)
        self.reset = Gtk.Button(label="Use the game's list")
        self.reset.connect("clicked", self.on_reset)
        buttons.pack_start(self.reset, False, False, 0)
        self.save = Gtk.Button(label="Save")
        self.save.connect("clicked", self.on_save)
        buttons.pack_end(self.save, False, False, 0)

    def field(self, form, row, label, entry):
        title = Gtk.Label(label=label, xalign=1)
        title.set_mnemonic_widget(entry)
        entry.get_accessible().set_name(label)
        form.attach(title, 0, row, 1, 1)
        form.attach(entry, 1, row, 2 if row < 2 else 1, 1)
        return entry

    def check_form(self):
        problem = gateway_problem(self.host.get_text().strip(), self.name.get_text())
        typed = self.host.get_text() or self.name.get_text()
        self.form_problem.set_text(problem if typed else "")
        self.add_button.set_sensitive(not problem)

    def on_add(self, button):
        self.offset.update()
        gateway = Gateway(True, self.host.get_text().strip(), str(self.offset.get_value_as_int()),
                          self.name.get_text().strip())
        self.list = select(self.list, -1) + [gateway]
        self.host.set_text("")
        self.name.set_text("")
        self.offset.set_value(0)
        self.show_list()

    def on_remove(self, button, number):
        self.list = remove(self.list, number)
        self.show_list()

    def on_select(self, button, number):
        if button.get_active():
            self.list = select(self.list, number)
            self.update()

    def on_save(self, button):
        self.saving = True
        self.update()
        self.launcher.set_gateways(self.edition, self.list, self.on_saved)
        self.problem.set_text("")

    def on_reset(self, button):
        self.ask("Use the game's list?", "Your own list of servers is deleted.", "Keep my list", "Use the game's list",
                 lambda: self.on_save_reset())

    def on_save_reset(self):
        self.saving = True
        self.update()
        self.launcher.reset_gateways(self.edition, self.on_saved)

    def on_saved(self, result):
        self.saving = False
        self.update()
        if result.code == 0:
            self.load(self.edition)
        else:
            self.problem.set_text(explain(result.code) + "\n" + result.error.strip())

    def load(self, edition):
        self.edition = edition
        self.title.set_markup("<b>Servers for the %s edition</b>" % edition.capitalize())
        self.launcher.gateways(edition, self.loaded)

    def loaded(self, found, result):
        if found is not None and found != self.saved:
            dirty = self.changed()
            self.saved = found
            if not dirty:
                self.show_list(found)
        self.update()

    def after_map(self):
        self.update()
        self.busy.set_visible(not self.usable)
        self.absent.set_visible(bool(self.absent.get_text()))
        self.editor.set_visible(not self.absent.get_text())

    def changed(self):
        return self.list != self.saved

    def refresh(self):
        """Looks at the list again, if the tab is showing."""
        if self.get_mapped() and self.edition:
            self.launcher.gateways(self.edition, self.loaded)

    def show_state(self, edition, status):
        """What the launcher reported about the chosen edition, or None."""
        installed = not status or status[edition] != "absent"
        self.absent.set_text("" if installed else "Install the %s edition first." % edition.capitalize())
        self.absent.set_visible(not installed)
        self.editor.set_visible(installed)
        usable = bool(status) and status[edition + "_in_use"] == "no"
        if usable != self.usable:
            self.usable = usable
            self.busy.set_visible(not usable and bool(status))
        self.update()

    def update(self):
        editable = self.usable and not self.saving
        for widget in (self.rows, self.form, self.reset, self.discard):
            widget.set_sensitive(editable)
        self.save.set_sensitive(self.changed() and editable)
        self.discard.set_visible(self.changed() and not self.saving)
        self.unsaved.set_text("Saving…" if self.saving else "Unsaved changes")
        self.unsaved.set_visible(self.changed() or self.saving)

    def show_list(self, found=None):
        if found is not None:
            self.list = found
        for row in self.rows.get_children():
            self.rows.remove(row)
        first = None
        for gateway in self.list:
            button = Gtk.RadioButton.new_with_label_from_widget(first, "%s (%s)" % (gateway.name, gateway.host))
            first = first or button
            button.set_active(gateway.selected)
            number = len(self.rows.get_children())
            button.connect("toggled", self.on_select, number)
            remove = Gtk.Button.new_from_icon_name("list-remove-symbolic", Gtk.IconSize.BUTTON)
            remove.get_accessible().set_name("Remove " + gateway.name)
            remove.set_tooltip_text("Remove")
            remove.set_sensitive(len(self.list) > 1)
            remove.connect("clicked", self.on_remove, number)
            row = Gtk.Box(spacing=12)
            row.pack_start(button, True, True, 0)
            row.pack_end(remove, False, False, 0)
            self.rows.add(row)
        self.rows.show_all()
        self.update()
