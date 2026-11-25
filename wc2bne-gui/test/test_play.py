import os
import shutil
import subprocess

from test.harness import WindowTest, wait_for


class Play(WindowTest):
    def test_shows_the_edition_and_mode_in_use(self):
        self.status(edition="official", mode="window", official="installed")
        self.open()
        self.wait_chosen("Official")
        self.wait_chosen("Window")

    def test_finds_the_launcher_in_the_project_beside_it(self):
        del self.env["WC2BNE"]
        os.mkdir(self.path("repo", "wc2bne-linux"))
        self.stand_in(self.path("repo", "wc2bne-linux", "wc2bne"))
        self.status(mode="window")
        self.open()
        self.wait_chosen("Window")

    def test_finds_the_launcher_on_path(self):
        del self.env["WC2BNE"]
        self.stand_in(os.path.join(self.bin, "wc2bne"))
        self.status(mode="window")
        self.open()
        self.wait_chosen("Window")

    def test_explains_when_there_is_no_launcher(self):
        del self.env["WC2BNE"]
        self.open()
        self.wait_text("wc2bne wasn't found")
        self.click("Close")
        self.assertEqual(self.processes[0].wait(10), 1)

    def test_an_unusable_WC2BNE_is_an_error_even_if_another_could_be_found(self):
        self.env["WC2BNE"] = self.path("nowhere")
        self.stand_in(os.path.join(self.bin, "wc2bne"))
        self.open()
        self.wait_text("wc2bne wasn't found")
        self.wait_text(self.path("nowhere"))

    def test_without_gtk_it_names_what_is_missing(self):
        os.mkdir(self.path("no-gtk"))
        with open(self.path("no-gtk", "gi.py"), "w") as f:
            f.write("raise ImportError('no gi')")
        self.env["PYTHONPATH"] = self.path("no-gtk")
        process = self.start()
        output = process.communicate(timeout=10)[0]
        self.assertEqual(process.returncode, 4)
        self.assertIn("GTK 3", output)
        self.assertIn("python3-gi", output)
        self.assertNotIn("Traceback", output)
        self.processes.remove(process)

    def test_choosing_an_edition_saves_it(self):
        self.status(official="installed")
        self.open()
        self.wait_chosen("Community")
        self.click("Official")
        self.wait_ran("config", "set", "edition", "official")
        self.wait_chosen("Official")

    def test_follows_changes_made_in_a_terminal(self):
        self.status(mode="window")
        self.open()
        self.wait_chosen("Window")  # not the first choice, so it came from status
        self.status(mode="fullscreen")
        self.wait_chosen("Fullscreen")

    def test_a_choice_stays_when_the_environment_decides_what_status_says(self):
        self.status(official="installed")
        self.open()
        self.wait_chosen("Community")
        self.click("Official")
        self.wait_ran("config", "set", "edition", "official")
        self.wait_checks()
        self.assertTrue(self.chosen("Official"))

    def test_play_starts_the_chosen_edition_and_mode(self):
        self.status(official="installed")
        self.open()
        self.wait_chosen("Community")
        self.click("Official")
        self.click("Window")
        self.click("Play")
        self.wait_ran("play", "--edition", "official", "--window")

    def test_says_running_while_the_game_runs_and_is_ready_again_after(self):
        self.answer("play", hold=True)
        self.open()
        self.click("Play")
        self.wait_text("Running")
        self.wait_disabled("Play")
        self.assertFalse(self.enabled("Window"))
        self.release("play")
        self.wait_text("Ready")
        self.wait_enabled("Play")

    def test_notices_a_game_started_in_a_terminal(self):
        self.open()
        self.wait_enabled("Play")
        self.status(community_in_use="play")
        self.wait_text("Running")
        self.wait_disabled("Play")
        self.status(community_in_use="no")
        self.wait_enabled("Play")

    def test_says_what_else_is_using_the_edition(self):
        self.status(community_in_use="gateway")
        self.open()
        self.wait_text("In use by: wc2bne gateway")
        self.wait_disabled("Play")

    def test_the_other_edition_can_be_played_meanwhile(self):
        self.status(official="installed", community_in_use="play")
        self.open()
        self.wait_text("Running")
        self.click("Official")
        self.click("Play")
        self.wait_ran("play", "--edition", "official", "--fullscreen")

    def test_closing_the_window_leaves_the_game_running(self):
        self.answer("play", hold=True, output="x" * 100000)
        self.open()
        self.click("Play")
        self.wait_ran("play", "--edition", "community", "--fullscreen")
        self.close()
        self.assertEqual(self.processes[0].wait(10), 0)
        self.assertFalse(os.path.exists(self.path("play-ended")))
        self.release("play")
        wait_for(lambda: os.path.exists(self.path("play-ended")), "the game to end by itself")

    def test_explains_why_the_game_did_not_start(self):
        for code, explanation in ((1, "It didn't work"), (2, "A setting isn't valid"), (3, "isn't installed"),
                                  (4, "A program it needs is missing"), (5, "A game file has changed"),
                                  (6, "is in use")):
            with self.subTest(code=code):
                self.answer("play", code=code, error="wc2bne: reason %d\n" % code)
                self.open()
                self.click("Play")
                self.wait_text(explanation)
                self.wait_text("reason %d" % code)
                self.wait_enabled("Play")
                self.stop()
                self.processes.clear()

    def test_explains_when_the_launcher_cannot_give_its_status(self):
        self.answer("status", code=2, error="wc2bne: wc2bne.conf: invalid mode: tiny\n")
        self.open()
        self.wait_text("A setting isn't valid")
        self.wait_text("invalid mode: tiny")
        self.wait_disabled("Play")
        self.status()
        self.wait_enabled("Play")

    def test_explains_when_a_choice_cannot_be_saved(self):
        self.answer("config set", code=1, error="wc2bne: can't write wc2bne.conf\n")
        self.open()
        self.wait_enabled("Window")
        self.click("Window")
        self.wait_text("It didn't work")
        self.wait_text("can't write wc2bne.conf")

    def test_opens_the_game_folder_and_the_maps(self):
        self.open()
        self.click("Open game folder")
        self.wait_ran("open", "--edition", "community")
        self.click("Open maps")
        self.wait_ran("open", "--edition", "community", "--maps")

    def test_nothing_to_play_or_open_when_the_edition_is_absent(self):
        self.status(community="absent")
        self.open()
        self.wait_text("isn't installed")
        self.assertFalse(self.enabled("Open game folder"))
        self.assertFalse(self.enabled("Open maps"))
        self.assertIsNone(self.control("Play"))

    def test_a_second_start_uses_the_window_that_is_open(self):
        self.open()
        self.wait_enabled("Play")
        second = self.start()
        self.assertEqual(second.wait(10), 0)
        from gi.repository import Atspi
        self.assertEqual(sum(app.get_child_count() for app in self.apps()), 1)

    def test_refuses_to_run_as_root(self):
        if subprocess.call(["unshare", "-r", "true"], stderr=subprocess.DEVNULL) != 0:
            self.skipTest("can't pretend to be root here")
        process = self.start(before=[shutil.which("unshare"), "-r"])
        output = process.communicate(timeout=10)[0]
        self.processes.remove(process)
        self.assertEqual(process.returncode, 1)
        self.assertIn("Don't run this as root", output)

    def test_quick_choices_are_saved_one_after_the_other(self):
        self.status(official="installed")
        self.answer("config set", hold=True)
        self.open()
        self.wait_enabled("Window")
        self.click("Window")
        self.wait_ran("config", "set", "mode", "window")
        self.click("Fullscreen")
        self.wait_chosen("Fullscreen")
        self.wait_checks()
        self.assertEqual(len(self.ran()), 1)
        self.release("config set")
        self.wait_ran("config", "set", "mode", "fullscreen")
        self.assertTrue(self.chosen("Fullscreen"))
