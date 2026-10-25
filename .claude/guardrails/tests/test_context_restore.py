"""Run with: python3 .claude/guardrails/tests/test_context_restore.py"""
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

GUARDRAILS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUARDRAILS))

import runner  # noqa: E402

RULE = runner.load_config()["rules"]["context_restore"]


class CarryTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        p = mock.patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": str(self.root)})
        p.start()
        self.addCleanup(p.stop)
        os.environ.pop("GUARDRAILS_OFF", None)

    def carry(self, text):
        (self.root / ".claude").mkdir(exist_ok=True)
        (self.root / ".claude/carry.md").write_text(text)

    def start(self, source="compact", **opts):
        config = {"rules": {"context_restore": {**RULE, **opts}}}
        denials, warnings, _ = runner.evaluate({"hook_event_name": "SessionStart", "source": source}, config)
        out = runner.render("SessionStart", denials, warnings)
        return out and out["hookSpecificOutput"]["additionalContext"]

    def test_replays_the_carry_file(self):
        self.carry("- review round 2 of 3\n- WAL spike: fine\n")
        ctx = self.start()
        self.assertIn("Notes you chose to carry", ctx)
        self.assertIn("review round 2 of 3", ctx)
        self.assertNotIn("[context_restore]", ctx)

    def test_on_resume_and_startup_but_not_clear(self):
        self.carry("x")
        self.assertTrue(self.start(source="resume"))
        self.assertTrue(self.start(source="startup"))
        self.assertIsNone(self.start(source="clear"))

    def test_missing_or_empty_is_silent(self):
        self.assertIsNone(self.start())
        self.carry("  \n")
        self.assertIsNone(self.start())

    def test_capped(self):
        self.carry("x" * 50)
        ctx = self.start(carry_limit=20)
        self.assertIn("x" * 20, ctx)
        self.assertNotIn("x" * 21, ctx)
        self.assertIn("30 characters over", ctx)

    def test_wired_in_settings(self):
        settings = json.loads((GUARDRAILS.parent / "settings.json").read_text())["hooks"]
        groups = [g for g in settings.get("SessionStart", [])
                  if any("guardrails/runner.py" in h.get("command", "") for h in g["hooks"])]
        matchers = "|".join(g.get("matcher", "") for g in groups)
        for source in ("compact", "resume", "startup"):
            self.assertIn(source, matchers)


if __name__ == "__main__":
    unittest.main()
