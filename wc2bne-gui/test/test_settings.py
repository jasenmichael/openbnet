import os

from test.harness import DOCTOR, WindowTest, wait_for


class Settings(WindowTest):
    def setUp(self):
        super().setUp()
        self.answer("config get data-folder", "/games/war2\n")
        self.answer("config get port", "6112\n")
        self.answer("--version", "wc2bne 0.1.0\n")

    def open_settings(self):
        self.open()
        self.wait_enabled("Play")
        self.tab("Settings")

    def test_shows_the_data_folder_and_can_switch_to_another(self):
        self.open_settings()
        wait_for(lambda: self.text_of("Data folder") == "/games/war2", "the data folder")
        self.wait_text("Nothing is moved")
        self.type("Data folder", "/mnt/big disk/war2")
        self.click("Use this folder")
        self.wait_ran("config", "set", "data-folder", "/mnt/big disk/war2")

    def test_can_go_back_to_the_default_data_folder(self):
        self.open_settings()
        self.click("Use the default folder")
        self.wait_ran("config", "set", "data-folder", "default")

    def test_says_when_WC2BNE_HOME_decides_the_data_folder(self):
        self.env["WC2BNE_HOME"] = "/elsewhere"
        self.open_settings()
        self.wait_text("WC2BNE_HOME environment variable decides it")
        self.wait_disabled("Use this folder")
        self.assertFalse(self.enabled("Use the default folder"))

    def test_saves_the_udp_port_of_the_chosen_edition(self):
        self.open_settings()
        wait_for(lambda: self.text_of("UDP port") == "6112", "the port")
        self.type("UDP port", "6200")
        self.click("Save port")
        self.wait_ran("config", "set", "port", "6200", "--edition", "community")
        self.wait_text("Saved")

    def test_an_invalid_port_is_explained(self):
        self.answer("config set", code=2, error="wc2bne: invalid setting: port 99999 (see: wc2bne help)\n")
        self.open_settings()
        self.click("Save port")
        self.wait_text("A setting isn't valid")
        self.wait_text("1024 to 65435")

    def test_lists_the_required_programs_and_checks_again(self):
        self.answer("doctor", DOCTOR.replace("xrandr=ok", "xrandr=optional"))
        self.open_settings()
        self.wait_text("bubblewrap: installed")
        self.wait_text("xrandr: not installed (optional)")
        self.answer("doctor", DOCTOR)
        self.click("Check again")
        self.wait_text("xrandr: installed")

    def test_port_and_uninstall_wait_while_the_edition_is_in_use_or_absent(self):
        self.status(community_in_use="play")
        self.open()
        self.wait_text("Running")
        self.tab("Settings")
        self.wait_disabled("Save port")
        self.wait_disabled("Uninstall the Community edition")
        self.status(community="absent")
        self.wait_disabled("Uninstall the Community edition")
        self.assertFalse(self.enabled("Save port"))
        self.status()
        self.wait_enabled("Save port")
        self.wait_enabled("Uninstall the Community edition")

    def test_uninstalls_after_saying_what_is_deleted_and_what_stays(self):
        self.open_settings()
        self.click("Uninstall the Community edition")
        self.wait_text("Shared maps, the installer and the other edition stay.")
        self.click("Keep it")
        self.wait_gone("Keep it")
        self.assertEqual(self.ran(), [])
        self.click("Uninstall the Community edition")
        self.status(community="absent")
        self.click("Uninstall", dialog=True)
        self.wait_ran("uninstall", "--edition", "community", "--yes")
        self.wait_disabled("Uninstall the Community edition")

    def test_a_broken_settings_file_can_be_repaired(self):
        self.answer("status", code=2, error="wc2bne: wc2bne.conf: invalid mode: tiny\n")
        self.open()
        self.wait_text("invalid mode: tiny")
        self.tab("Settings")
        self.click("Repair settings")
        self.wait_ran("config", "set", "edition", "community")
        self.wait_ran("config", "set", "mode", "fullscreen")

    def test_shows_the_versions_and_the_license(self):
        self.open_settings()
        self.wait_text("wc2bne-gui 0.1.0, wc2bne 0.1.0")
        self.wait_text("GPL version 3 or later")
