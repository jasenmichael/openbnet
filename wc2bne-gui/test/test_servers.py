from test.harness import WindowTest, wait_for

LIST = "1\t1\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server\n"


class Servers(WindowTest):
    def setUp(self):
        super().setUp()
        self.answer("gateway list", LIST)

    def saved(self):
        """What the last Save sent."""
        with open(self.path("input-gateway-set"), encoding="utf-8") as f:
            return f.read()

    def open_servers(self):
        self.open()
        self.wait_enabled("Play")
        self.tab("Servers")

    def test_shows_the_gateways_and_which_is_selected(self):
        self.open_servers()
        self.wait_chosen("One (one.example)")
        self.assertFalse(self.chosen("Two Server (two.example)"))
        self.assertIn(["gateway", "list", "--porcelain", "--edition", "community"], self.all_ran())

    def test_selecting_one_and_saving_sends_the_whole_list(self):
        self.open_servers()
        self.click("Two Server (two.example)")
        self.click("Save")
        self.wait_ran("gateway", "set", "--edition", "community")
        self.assertEqual(self.saved(), "1\t0\tone.example\t-3\tOne\n2\t1\ttwo.example\t0\tTwo Server\n")

    def test_adds_a_gateway_and_selects_it(self):
        self.open_servers()
        self.type("Address", "my.server.example")
        self.type("Name", "Mine")
        self.type("GMT offset", "-5")
        self.click("Add")
        self.wait_chosen("Mine (my.server.example)")
        self.click("Save")
        self.wait_ran("gateway", "set", "--edition", "community")
        self.assertEqual(self.saved(), "1\t0\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server\n"
                                       "3\t1\tmy.server.example\t-5\tMine\n")

    def test_explains_an_invalid_address_or_name_before_adding(self):
        self.open_servers()
        self.type("Address", "bad host;")
        self.type("Name", "Fine")
        self.wait_text("An address may only have letters, digits, dots and dashes.")
        self.wait_disabled("Add")
        self.type("Address", "good.example")
        self.type("Name", "a|b")
        self.wait_text("A name can't contain | or \\.")
        self.wait_disabled("Add")
        self.type("Name", "ok")
        self.wait_enabled("Add")

    def test_removes_a_gateway_but_never_the_last(self):
        self.open_servers()
        self.click("Remove One")
        self.wait_gone("One (one.example)")
        self.wait_chosen("Two Server (two.example)")
        self.wait_disabled("Remove Two Server")
        self.click("Save")
        self.wait_ran("gateway", "set", "--edition", "community")
        self.assertEqual(self.saved(), "1\t1\ttwo.example\t0\tTwo Server\n")

    def test_shows_unsaved_changes_and_can_discard_them(self):
        self.open_servers()
        self.assertFalse(self.enabled("Save"))
        self.click("Two Server (two.example)")
        self.wait_text("Unsaved changes")
        self.click("Discard changes")
        self.wait_chosen("One (one.example)")
        self.wait_disabled("Save")
        self.assertFalse(any("Unsaved" in text for text in self.shown()))

    def test_a_failed_save_keeps_the_changes_and_says_why(self):
        self.answer("gateway set", code=6, error="wc2bne: the community edition is in use (play); close it first\n")
        self.open_servers()
        self.click("Two Server (two.example)")
        self.click("Save")
        self.wait_text("This edition is in use.")
        self.wait_text("close it first")
        self.assertTrue(self.chosen("Two Server (two.example)"))
        self.wait_text("Unsaved changes")

    def test_going_back_to_the_games_list_asks_first(self):
        self.open_servers()
        self.click("Use the game's list")
        self.click("Keep my list")
        self.wait_gone("Keep my list")
        self.assertEqual(self.ran(), [])
        self.answer("gateway list", "1\t1\tgame.example\t0\tGame\n")
        self.click("Use the game's list")
        self.click("Use the game's list", dialog=True)
        self.wait_ran("gateway", "reset", "--edition", "community")
        self.wait_chosen("Game (game.example)")

    def test_read_only_while_the_edition_is_in_use(self):
        self.status(community_in_use="play")
        self.open()
        self.wait_text("Running")
        self.tab("Servers")
        self.wait_text("Changes can be made when the game has closed.")
        self.wait_disabled("Two Server (two.example)")
        self.assertFalse(self.enabled("Add"))
        self.assertFalse(self.enabled("Use the game's list"))
        self.status()
        self.wait_enabled("Two Server (two.example)")

    def test_says_the_edition_must_be_installed_first(self):
        self.status(community="absent")
        self.open()
        self.wait_enabled("Install")
        self.tab("Servers")
        self.wait_text("Install the Community edition first.")
        self.assertIsNone(self.control("Add"))

    def test_names_in_any_language_are_shown_and_saved_unchanged(self):
        self.answer("gateway list", "1\t1\tone.example\t0\t<b>Ørkenen</b> & 戦場\n")
        self.open_servers()
        self.wait_chosen("<b>Ørkenen</b> & 戦場 (one.example)")
        self.type("Address", "two.example")
        self.type("Name", "Сервер <i>")
        self.click("Add")
        self.click("Save")
        self.wait_ran("gateway", "set", "--edition", "community")
        self.assertEqual(self.saved(), "1\t0\tone.example\t0\t<b>Ørkenen</b> & 戦場\n2\t1\ttwo.example\t0\tСервер <i>\n")

    def test_shows_the_list_of_the_chosen_edition(self):
        self.status(official="installed")
        self.open_servers()
        self.wait_chosen("One (one.example)")
        self.tab("Play")
        self.click("Official")
        self.answer("gateway list", "1\t1\tglobal.example\t8\tGlobal\n")
        self.tab("Servers")
        self.wait_chosen("Global (global.example)")
        self.assertIn(["gateway", "list", "--porcelain", "--edition", "official"], self.all_ran())
        self.wait_text("Servers for the Official edition")

    def test_follows_changes_made_in_a_terminal_unless_there_are_unsaved_ones(self):
        self.open_servers()
        self.wait_chosen("One (one.example)")
        self.answer("gateway list", "1\t0\tone.example\t-3\tOne\n2\t1\ttwo.example\t0\tTwo Server\n")
        self.wait_chosen("Two Server (two.example)")
        self.click("One (one.example)")
        self.answer("gateway list", "1\t1\tnew.example\t0\tNew\n")
        self.wait_checks()
        self.wait_checks()
        self.assertTrue(self.chosen("One (one.example)"))

    def test_asks_before_unsaved_changes_are_lost(self):
        self.status(official="installed")
        self.open_servers()
        self.click("Two Server (two.example)")
        self.tab("Play")
        self.click("Official")
        self.click("Keep editing")
        self.wait_chosen("Community")
        self.assertNotIn(["config", "set", "edition", "official"], self.ran())
        self.click("Official")
        self.click("Discard changes", dialog=True)
        self.wait_ran("config", "set", "edition", "official")

    def test_asks_before_closing_with_unsaved_changes(self):
        self.open_servers()
        self.click("Two Server (two.example)")
        self.close()
        self.click("Keep editing")
        self.wait_gone("Keep editing")
        self.close()
        self.click("Close without saving")
        self.assertEqual(self.processes[0].wait(10), 0)

    def test_reminds_that_servers_can_run_code(self):
        self.open_servers()
        self.wait_text("Add only servers you trust")

    def test_shows_that_it_is_saving(self):
        self.answer("gateway set", hold=True)
        self.open_servers()
        self.click("Two Server (two.example)")
        self.click("Save")
        self.wait_text("Saving")
        self.wait_disabled("Save")
        self.assertFalse(self.enabled("Two Server (two.example)"))
        self.answer("gateway list", "1\t0\tone.example\t-3\tOne\n2\t1\ttwo.example\t0\tTwo Server\n")
        self.release("gateway set")
        self.wait_enabled("Two Server (two.example)")
        self.wait_gone("Saving…")
