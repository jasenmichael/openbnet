"""Runs every test, or those named: python3 -m test [test.test_play.Play.test_...]"""
import sys
import unittest

from test import harness

if __name__ == "__main__":
    sys.argv[0:1] = ["-m", "test"] if not sys.argv[0].startswith("-") else sys.argv[0:1]
    harness.enter_private_session()
    import gi
    gi.require_version("Atspi", "2.0")
    names = [a for a in sys.argv[2:] if not a.startswith("-")]
    if names:
        suite = unittest.defaultTestLoader.loadTestsFromNames(names)
    else:
        suite = unittest.defaultTestLoader.discover("test", top_level_dir=".")
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
