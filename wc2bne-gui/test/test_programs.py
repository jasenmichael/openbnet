import os

from test.harness import DOCTOR, WindowTest, wait_for

MISSING = DOCTOR.replace("wine32=ok", "wine32=missing").replace("xrandr=ok", "xrandr=optional")
COMMANDS = "install=dpkg --add-architecture i386\ninstall=apt-get install -y wine32:i386\n"


class Programs(WindowTest):
    def setUp(self):
        super().setUp()
        self.answer("doctor", MISSING + COMMANDS, code=4)
        self.stand_in(os.path.join(self.bin, "pkexec"))

    def test_shows_what_is_missing_and_the_exact_commands_without_running_them(self):
        self.open()
        self.wait_text("Wine's 32-bit libraries")
        self.wait_text("dpkg --add-architecture i386\napt-get install -y wine32:i386")
        self.assertFalse(any("xrandr" in text for text in self.shown()))
        self.assertEqual(self.ran(), [])

    def test_installs_them_with_one_password_prompt_then_checks_again(self):
        self.answer("pkexec", hold=True)
        self.open()
        self.click("Install programs")
        self.wait_ran("pkexec", "/bin/sh", "-c",
                      r"set -e\ndpkg --add-architecture i386\napt-get install -y wine32:i386")
        self.wait_disabled("Install programs")
        self.answer("doctor", DOCTOR)
        self.release("pkexec")
        self.wait_gone("Install programs")
        self.assertEqual(len(self.ran()), 1)

    def test_refusing_the_password_prompt_is_harmless(self):
        self.answer("pkexec", code=126, error="Error: Request dismissed\n")
        self.open()
        self.click("Install programs")
        self.wait_text("Permission wasn't given, so nothing was installed.")
        self.wait_enabled("Install programs")
        self.wait_enabled("Play")

    def test_says_when_they_are_still_missing_and_why(self):
        self.answer("pkexec", code=100, output="Reading package lists...\n", error="E: Unable to locate package\n")
        self.open()
        self.click("Install programs")
        self.wait_text("Some programs are still missing.")
        self.wait_text("E: Unable to locate package")

    def test_copies_the_commands(self):
        self.open()
        self.click("Copy commands")
        wait_for(lambda: self.clipboard() == "dpkg --add-architecture i386\napt-get install -y wine32:i386",
                 "the commands on the clipboard")

    def test_without_pkexec_the_commands_can_still_be_copied(self):
        os.remove(os.path.join(self.bin, "pkexec"))
        self.open()
        self.wait_enabled("Copy commands")
        self.assertIsNone(self.control("Install programs"))
        self.wait_text("as root")

    def test_on_an_unknown_distro_it_only_names_them(self):
        self.answer("doctor", MISSING, code=4)
        self.open()
        self.wait_text("Wine's 32-bit libraries")
        self.assertIsNone(self.control("Install programs"))
        self.assertIsNone(self.control("Copy commands"))

    def test_checks_the_programs_again_when_one_turns_out_to_be_missing(self):
        self.answer("doctor", DOCTOR)
        self.answer("play", code=4, error="wc2bne: missing: wine32\n")
        self.open()
        self.wait_enabled("Play")
        self.answer("doctor", MISSING + COMMANDS, code=4)
        self.click("Play")
        self.wait_enabled("Install programs")
