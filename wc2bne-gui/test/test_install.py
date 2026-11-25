import os
import time

from test.harness import WindowTest, wait_for


class Install(WindowTest):
    def setUp(self):
        super().setUp()
        self.status(community="absent")

    def test_an_absent_edition_can_be_installed(self):
        self.open()
        self.click("Install")
        self.wait_ran("install", "--edition", "community", "--porcelain")

    def test_shows_the_step_and_how_far_the_download_is(self):
        self.answer("install", output="step=download\npercent=42\n", hold=True)
        self.open()
        self.click("Install")
        self.wait_text("Downloading the installer: 42%")
        self.wait_disabled("Install")
        self.assertFalse(self.enabled("Official"))
        self.more("install", "step=prepare\n")
        self.wait_text("Preparing Wine")
        self.more("install", "step=install\n")
        self.wait_text("Installing the game")
        self.more("install", "step=finish\n")
        self.wait_text("Finishing")

    def test_is_ready_to_play_as_soon_as_it_is_installed(self):
        self.answer("install", output="step=download\n", hold=True)
        self.open()
        self.click("Install")
        self.wait_text("Downloading the installer")
        self.status()
        before = time.monotonic()
        self.release("install")
        self.wait_enabled("Play")
        self.assertLess(time.monotonic() - before, 1)  # not at the next regular check
        self.assertEqual(len(self.ran()), 1)

    def test_cancelling_stops_the_launcher_and_says_so(self):
        self.answer("install", output="step=download\npercent=3\n", hold=True)
        self.open()
        self.assertIsNone(self.control("Cancel"))
        self.click("Install")
        self.wait_text("3%")
        self.click("Cancel")
        self.wait_text("Cancelled")
        self.assertTrue(os.path.exists(self.path("interrupted")))
        self.wait_enabled("Install")
        self.wait_gone("Cancel")

    def test_closing_asks_before_stopping_an_install(self):
        self.answer("install", output="step=download\n", hold=True)
        self.open()
        self.click("Install")
        self.wait_text("Downloading")
        self.close()
        self.click("Keep installing")
        self.wait_gone("Keep installing")
        self.assertFalse(os.path.exists(self.path("interrupted")))
        self.close()
        self.click("Stop and close")
        self.assertEqual(self.processes[0].wait(10), 0)
        wait_for(lambda: os.path.exists(self.path("interrupted")), "the install to be interrupted")

    def test_a_failed_install_is_explained_and_can_be_tried_again(self):
        for code, explanation in ((1, "It didn't work"), (4, "A program it needs is missing"),
                                  (5, "wasn't run"), (6, "is in use")):
            with self.subTest(code=code):
                self.answer("install", code=code, error="wc2bne: reason %d\n" % code)
                self.open()
                self.click("Install")
                self.wait_text(explanation)
                self.wait_text("reason %d" % code)
                self.click("Install")
                wait_for(lambda: len(self.ran()) == 2, "a second try")
                self.stop()
                self.processes.clear()
                os.remove(self.path("ran"))

    def test_changed_game_files_offer_a_reinstall_that_warns_first(self):
        self.status(community="changed")
        self.open()
        self.wait_text("A game file has changed")
        self.click("Reinstall")
        self.wait_text("saved games")
        self.click("Keep it")
        self.wait_gone("Keep it")
        self.assertEqual(self.ran(), [])
        self.click("Reinstall")
        self.click("Delete and reinstall")
        self.wait_ran("install", "--edition", "community", "--porcelain")
        self.assertEqual(self.ran()[0], ["uninstall", "--edition", "community", "--yes"])

    def test_reinstalls_the_edition_it_deleted_even_if_another_is_chosen_meanwhile(self):
        self.status(community="changed", official="installed")
        self.answer("uninstall", hold=True)
        self.open()
        self.click("Reinstall")
        self.click("Delete and reinstall")
        self.wait_ran("uninstall", "--edition", "community", "--yes")
        self.wait_text("Removing")
        self.assertFalse(self.enabled("Official"))
        self.release("uninstall")
        self.wait_ran("install", "--edition", "community", "--porcelain")
