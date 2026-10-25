"""Run with: python3 .claude/guardrails/tests/test_honesty.py"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

GUARDRAILS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUARDRAILS))

import runner  # noqa: E402

CONFIG = runner.load_config()


def only(*names):
    return {"rules": {n: CONFIG["rules"][n] for n in names}}


def pre(tool, tool_input, **extra):
    return {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": tool_input, **extra}


def bash(command, **extra):
    return pre("Bash", {"command": command}, **extra)


class Repo(unittest.TestCase):
    """A throwaway git repo with an upstream, used as CLAUDE_PROJECT_DIR."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        top = Path(self.tmp.name)
        self.root, remote = top / "work", top / "remote.git"
        self.git_run("init", "-q", "--bare", str(remote), cwd=top)
        self.git_run("init", "-q", "-b", "main", str(self.root), cwd=top)
        for k, v in (("user.email", "t@t"), ("user.name", "t"), ("commit.gpgsign", "false")):
            self.git("config", k, v)
        self.write("README.md", "hi\n")
        self.commit("init")
        self.git("remote", "add", "origin", str(remote))
        self.git("push", "-q", "-u", "origin", "main")
        patches = [mock.patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": str(self.root)}),
                   mock.patch.object(runner, "LOG_PATH", top / "log.jsonl")]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        os.environ.pop("GUARDRAILS_OFF", None)

    def git_run(self, *args, cwd):
        subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True)

    def git(self, *args):
        self.git_run(*args, cwd=self.root)

    def write(self, rel, text):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def commit(self, msg="c"):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", msg)

    def decide(self, hook, config):
        hook = {"cwd": str(self.root), **hook}
        denials, warnings, _ = runner.evaluate(hook, config)
        return runner.render(hook["hook_event_name"], denials, warnings)

    def denied(self, hook, config, *fragments):
        out = self.decide(hook, config)
        self.assertIsNotNone(out, "expected a deny, got silence")
        reason = out["hookSpecificOutput"]["permissionDecisionReason"]
        for f in fragments:
            self.assertIn(f, reason)
        return reason

    def silent(self, hook, config):
        self.assertIsNone(self.decide(hook, config))


def features(*items):
    return json.dumps({"features": [dict(i) for i in items]}, indent=2)


class ProtectedPathsTests(Repo):
    C = only("protected_paths")

    def test_edit_locked_denied(self):
        for path in ("KICKOFF.md", ".claude/settings.json", ".claude/guardrails/config.toml",
                     ".claude/guardrails/rules/new_rule.py"):
            self.denied(pre("Edit", {"file_path": str(self.root / path), "old_string": "a", "new_string": "b"}),
                        self.C, "human-owned")

    def test_ordinary_files_fine(self):
        self.silent(pre("Write", {"file_path": str(self.root / "src/app.ts"), "content": "x"}), self.C)
        self.silent(pre("Write", {"file_path": str(self.root / ".claude/skills/x/SKILL.md"), "content": "x"}), self.C)

    def test_truth_write_once(self):
        truth = {"file_path": str(self.root / "examples/TRUTH.json"), "content": "{}"}
        self.silent(pre("Write", truth), self.C)  # creating it is fine
        self.write("examples/TRUTH.json", "{}")
        self.denied(pre("Write", truth), self.C, "frozen")

    def test_shell_writes_denied(self):
        for cmd in ("echo x > KICKOFF.md", "sed -i 's/a/b/' KICKOFF.md", "cat a | tee .claude/settings.json",
                    "rm .claude/guardrails/runner.py", "git checkout -- KICKOFF.md", "cp /tmp/x KICKOFF.md",
                    "cd sub && echo x >> ../KICKOFF.md", "cat > KICKOFF.md <<'X'\nnew\nX"):
            self.denied(bash(cmd), self.C, "human-owned")

    def test_shell_reads_fine(self):
        for cmd in ("cat KICKOFF.md", "grep -n x KICKOFF.md > /tmp/out", "cp KICKOFF.md /tmp/copy",
                    "python3 .claude/guardrails/tests/test_guardrails.py", "git diff KICKOFF.md", "wc -w KICKOFF.md",
                    "cat > notes.md <<'X'\necho x > KICKOFF.md\nX"):
            self.silent(bash(cmd), self.C)

    def test_paths_outside_project_ignored(self):
        self.silent(pre("Write", {"file_path": "/tmp/KICKOFF.md", "content": "x"}), self.C)


class TestIntegrityTests(Repo):
    C = only("test_integrity")

    def test_new_skip_denied(self):
        self.write("src/a.test.ts", "it('works', () => { expect(1).toBe(1) })\n")
        self.denied(pre("Edit", {"file_path": str(self.root / "src/a.test.ts"),
                                 "old_string": "it('works'", "new_string": "it.skip('works'"}), self.C, "skip")

    def test_python_and_go_markers(self):
        self.denied(pre("Write", {"file_path": str(self.root / "tests/test_x.py"),
                                  "content": "@pytest.mark.skip\ndef test_x(): assert 1\n"}), self.C, "marker")
        self.denied(pre("Write", {"file_path": str(self.root / "svc/x_test.go"),
                                  "content": "func TestX(t *testing.T) { t.Skip() }\n"}), self.C, "marker")

    def test_existing_skip_untouched_is_fine(self):
        self.write("src/a.test.ts", "it.skip('old', () => {})\nit('b', () => { expect(1) })\n")
        self.silent(pre("Edit", {"file_path": str(self.root / "src/a.test.ts"),
                                 "old_string": "it('b'", "new_string": "it('c'"}), self.C)

    def test_assertion_drop_only_warns(self):
        self.write("src/a.test.ts", "expect(1)\nexpect(2)\n")
        out = self.decide(pre("Edit", {"file_path": str(self.root / "src/a.test.ts"),
                                       "old_string": "expect(2)\n", "new_string": ""}), self.C)
        hso = out["hookSpecificOutput"]
        self.assertNotIn("permissionDecision", hso)
        self.assertIn("removes 1 assertion", hso["additionalContext"])

    def test_non_test_files_ignored(self):
        self.silent(pre("Write", {"file_path": str(self.root / "src/a.ts"), "content": "it.only('x')"}), self.C)

    def test_feature_removed_or_tier_lowered(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        fl = str(self.root / "feature_list.json")
        self.denied(pre("Write", {"file_path": fl, "content": features()}), self.C, "S1 was removed")
        self.denied(pre("Write", {"file_path": fl, "content": features({"id": "S1", "tier": "B"})}),
                    self.C, "tier lowered")

    def test_pass_needs_existing_evidence(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        fl = str(self.root / "feature_list.json")
        self.denied(pre("Write", {"file_path": fl, "content": features({"id": "S1", "tier": "A", "passes": True})}),
                    self.C, "no `evidence`")
        self.denied(pre("Write", {"file_path": fl, "content": features(
            {"id": "S1", "tier": "A", "passes": True, "evidence": ["docs/evidence/s1.md"]})}), self.C, "doesn't exist")
        self.write("docs/evidence/s1.md", "ok")
        self.silent(pre("Write", {"file_path": fl, "content": features(
            {"id": "S1", "tier": "A", "passes": True, "evidence": ["docs/evidence/s1.md"]})}), self.C)

    def test_adding_entries_and_raising_tiers_fine(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "B"}))
        self.silent(pre("Write", {"file_path": str(self.root / "feature_list.json"), "content": features(
            {"id": "S1", "tier": "A"}, {"id": "X1", "tier": "C", "passes": True})}), self.C)


class ReleaseActionsTests(Repo):
    C = only("release_actions")

    def test_denied(self):
        for cmd in ("npm publish", "pnpm -r publish", "gh release create v0.1.0", "git push origin v0.1.0",
                    "git push --tags", "git push origin refs/tags/v1", "git push -f origin main",
                    "git push --force", "git tag -d v1", "gh repo edit --visibility public",
                    "git commit --no-verify -m x", "git push origin :main", "git push origin +main"):
            self.denied(bash(cmd), self.C, "human's call")

    def test_allowed(self):
        for cmd in ("npm publish --dry-run", "git push -u origin ccr-branch", "git push origin HEAD",
                    "git push --force-with-lease origin my-branch", "gh release view v1", "git tag",
                    "npm pack", "gh run list",
                    "echo '{\"command\":\"git push --tags\"}' | python3 runner.py",
                    "grep -n 'npm publish' docs/release.md",
                    "cat > notes.md <<'X'\ngh release create v1\ngit push --tags\nX\nls"):
            self.silent(bash(cmd), self.C)

    def test_hidden_in_a_chain_still_denied(self):
        for cmd in ("cd repo && git push --tags", "FOO=1 npm publish", "make build; gh release create v1",
                    "echo $(git push --tags)", "cat <<'X'\nhi\nX\ngit push --tags"):
            self.denied(bash(cmd), self.C, "human's call")


class CommitGateTests(Repo):
    C = only("commit_gate")

    def test_staged_locked_file(self):
        self.write("KICKOFF.md", "x")
        self.git("add", "KICKOFF.md")
        self.denied(bash("git commit -m x"), self.C, "human-owned")

    def test_commit_all_sees_tracked_changes(self):
        self.write("KICKOFF.md", "x")
        self.commit()  # pretend a person committed it
        self.write("KICKOFF.md", "changed by a script")
        self.silent(bash("git commit -m x"), self.C)  # not staged
        self.denied(bash("git commit -am x"), self.C, "human-owned")

    def test_frozen_truth(self):
        self.write("examples/TRUTH.json", "{}")
        self.git("add", "-A")
        self.silent(bash("git commit -m 'freeze truth'"), self.C)  # first commit is fine
        self.commit()
        self.write("examples/TRUTH.json", '{"x": 1}')
        self.git("add", "-A")
        self.denied(bash("git add -A && git commit -m y"), self.C, "frozen")

    def test_script_written_skip_and_feature_flip(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        self.write("src/a.test.ts", "it('a', () => expect(1))\n")
        self.commit()
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": True}))
        self.write("src/a.test.ts", "it.only('a', () => expect(1))\n")
        self.git("add", "-A")
        self.denied(bash("git commit -m z"), self.C, "no `evidence`", "skip/only")

    def test_global_options_do_not_hide_a_commit(self):
        self.write("KICKOFF.md", "x")
        self.git("add", "-A")
        for cmd in ("git -c user.name=x commit -m y", "git --no-pager commit -m y", "GIT_AUTHOR_NAME=x git commit -m y",
                    "/usr/bin/git -C . commit -m y", "make && git commit -m y"):
            self.denied(bash(cmd), self.C, "human-owned")

    def test_ordinary_commit_fine(self):
        self.write("src/a.ts", "x")
        self.git("add", "-A")
        self.silent(bash("git commit -m ok"), self.C)

    def test_non_commit_commands_ignored(self):
        self.write("KICKOFF.md", "x")
        self.git("add", "-A")
        self.silent(bash("git status"), self.C)


class StopAndRestoreTests(Repo):
    def stop(self, active=False):
        return self.decide({"hook_event_name": "Stop", "stop_hook_active": active}, only("honest_stop"))

    def start(self, source="compact"):
        return self.decide({"hook_event_name": "SessionStart", "source": source}, only("context_restore"))

    def test_inactive_without_feature_list(self):
        self.write("dirty.txt", "x")
        self.assertIsNone(self.stop())
        self.assertIsNone(self.start())

    def test_stop_blocks_on_dirty_unpushed_and_failing(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        self.commit()
        self.write("dirty.txt", "x")
        out = self.stop()
        self.assertEqual(out["decision"], "block")
        for frag in ("uncommitted", "aren't pushed", "S1", "## Blocked"):
            self.assertIn(frag, out["reason"])

    def test_stop_never_blocks_twice(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        self.assertIsNone(self.stop(active=True))

    def test_recorded_blocker_and_clean_tree_allow_stop(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        self.write("progress.md", "## Now\nx\n## Blocked\n- needs the human to approve the mods prompt\n")
        self.commit()
        self.git("push", "-q")
        self.assertIsNone(self.stop())

    def test_empty_blocked_section_does_not_count(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False}))
        self.write("progress.md", "## Blocked\nnone\n")
        self.commit()
        self.git("push", "-q")
        self.assertIn("## Blocked", self.stop()["reason"])

    def test_restore_card(self):
        self.write("feature_list.json", features({"id": "S1", "tier": "A", "passes": False},
                                                 {"id": "S5", "tier": "B", "passes": True}))
        self.write("progress.md", "# P\n## Now\nBuilding the store.\n## Later\nignore me\n")
        self.commit()
        ctx = self.start()["hookSpecificOutput"]["additionalContext"]
        for frag in ("Context restored after 'compact'", "KICKOFF.md is the contract", "Tier A: 0/1",
                     "Tier B: 1/1", "Failing tier A: S1", "Building the store.", "branch main"):
            self.assertIn(frag, ctx)
        self.assertNotIn("ignore me", ctx)
        self.assertIsNone(self.start(source="clear"))


class WiringTests(unittest.TestCase):
    def setUp(self):
        self.settings = json.loads((GUARDRAILS.parent / "settings.json").read_text())["hooks"]

    def routed(self, event):
        return [g for g in self.settings.get(event, [])
                if any("guardrails/runner.py" in h.get("command", "") for h in g["hooks"])]

    def test_session_start_and_stop_are_routed(self):
        self.assertTrue(self.routed("Stop"))
        matchers = [g.get("matcher", "") for g in self.routed("SessionStart")]
        for source in ("compact", "resume", "startup"):
            self.assertTrue(any(source in m or m in ("", "*") for m in matchers), source)

    def test_stop_hook_cannot_trap_a_session(self):
        for g in self.routed("Stop"):
            for h in g["hooks"]:
                self.assertNotEqual(h.get("onFailure"), "block")


if __name__ == "__main__":
    unittest.main()
