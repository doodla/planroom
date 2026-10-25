"""Run with: python3 .claude/guardrails/tests/test_guardrails.py"""
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

GUARDRAILS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUARDRAILS))

import runner  # noqa: E402
from rules import subagent_model_effort as sme  # noqa: E402

ENFORCE = {"rules": {"subagent_model_effort": {"mode": "enforce", "on_error": "closed"}}}


def pre(tool, tool_input, **extra):
    return {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": tool_input,
            "permission_mode": "auto", **extra}


def workflow(script):
    return pre("Workflow", {"script": textwrap.dedent(script)})


class IsolatedEnv(unittest.TestCase):
    """Point project and home dirs at temp dirs so real agent definitions don't leak in."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.project, self.home = root / "project", root / "home"
        self.project.mkdir()
        self.home.mkdir()
        patches = [
            mock.patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": str(self.project), "HOME": str(self.home)}),
            mock.patch.object(runner, "LOG_PATH", root / "logs" / "decisions.jsonl"),
        ]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        self.addCleanup(self.tmp.cleanup)
        os.environ.pop("GUARDRAILS_OFF", None)

    def define_agent(self, name, where="project", **fields):
        base = self.project / ".claude" if where == "project" else self.home / ".claude"
        d = base / "agents"
        d.mkdir(parents=True, exist_ok=True)
        fm = "\n".join(f"{k}: {v}" for k, v in {"name": name, **fields}.items())
        (d / f"{name}.md").write_text(f"---\n{fm}\n---\nYou are {name}.\n")

    def decide(self, hook, config=ENFORCE):
        denials, warnings, _ = runner.evaluate(hook, config)
        return runner.render(hook["hook_event_name"], denials, warnings)

    def assertDenied(self, hook, *fragments, config=ENFORCE):
        out = self.decide(hook, config)
        self.assertIsNotNone(out, "expected a deny, got silence")
        hso = out["hookSpecificOutput"]
        self.assertEqual(hso["permissionDecision"], "deny")
        for f in fragments:
            self.assertIn(f, hso["permissionDecisionReason"])
        return hso["permissionDecisionReason"]

    def assertSilent(self, hook, config=ENFORCE):
        self.assertIsNone(self.decide(hook, config))


class AgentToolTests(IsolatedEnv):
    def test_missing_both(self):
        self.assertDenied(pre("Agent", {"prompt": "x"}), "model and effort")

    def test_missing_effort(self):
        reason = self.assertDenied(pre("Agent", {"prompt": "x", "model": "sonnet"}), "must set effort")
        self.assertNotIn("model and", reason)

    def test_both_set(self):
        self.assertSilent(pre("Agent", {"prompt": "x", "model": "sonnet", "effort": "low"}))

    def test_legacy_task_tool_name(self):
        self.assertDenied(pre("Task", {"prompt": "x"}), "Task call")

    def test_fork_exempt(self):
        self.assertSilent(pre("Agent", {"prompt": "x", "subagent_type": "fork"}))

    def test_definition_pins_both(self):
        self.define_agent("scout", model="haiku", effort="low")
        self.assertSilent(pre("Agent", {"prompt": "x", "subagent_type": "scout"}))

    def test_definition_pins_model_only(self):
        self.define_agent("scout", model="haiku")
        self.assertDenied(pre("Agent", {"prompt": "x", "subagent_type": "scout"}), "must set effort")

    def test_definition_inherit_does_not_count(self):
        self.define_agent("scout", model="inherit", effort="low")
        self.assertDenied(pre("Agent", {"prompt": "x", "subagent_type": "scout"}), "must set model")

    def test_user_level_definition(self):
        self.define_agent("reviewer", where="home", model="opus", effort="high")
        self.assertSilent(pre("Agent", {"prompt": "x", "subagent_type": "reviewer"}))

    def test_unknown_or_builtin_type_needs_both(self):
        self.assertDenied(pre("Agent", {"prompt": "x", "subagent_type": "general-purpose"}), "model and effort")

    def test_other_tools_ignored(self):
        self.assertSilent(pre("Bash", {"command": "ls"}))


class WorkflowTests(IsolatedEnv):
    def test_all_calls_pinned(self):
        self.assertSilent(workflow("""
            const a = await agent('a', {model: 'opus', effort: 'high'})
            const b = await agent(`b ${x}`, {schema: S, effort: 'low', model: 'haiku'})
        """))

    def test_reports_each_bad_call_by_line(self):
        reason = self.assertDenied(workflow("""
            const a = await agent('ok', {model: 'opus', effort: 'high'})
            const b = await agent('no opts')
            const c = await agent('half', {model: 'sonnet'})
        """), "line 3: missing model, effort", "line 4: missing effort")
        self.assertNotIn("line 2", reason)

    def test_ignores_comments_strings_and_lookalikes(self):
        self.assertSilent(workflow("""
            // agent('commented out')
            /* agent('block comment') */
            const u = 'https://x.y // agent("in a string")'
            const w = await workflow('other')
            const s = await subagent('not ours')
            const t = await obj.agent('method call')
        """))

    def test_keys_inside_prompt_text_do_not_count(self):
        self.assertDenied(workflow("""
            await agent('use model: opus and effort: high', {schema: S})
        """), "line 2")

    def test_parens_inside_template_expressions(self):
        self.assertSilent(workflow("""
            await agent(`find (bugs) in ${f(g(x))}`, {model: 'sonnet', effort: 'low'})
        """))

    def test_nested_in_parallel(self):
        self.assertDenied(workflow("""
            await parallel(xs.map(x => () => agent(`v ${x}`, {schema: V})))
        """), "line 2")

    def test_nested_call_options_do_not_satisfy_outer_call(self):
        reason = self.assertDenied(workflow("""
            await agent(build(agent('x', {model: 'a', effort: 'b'})))
        """), "line 2: missing model, effort in agent(build(")
        self.assertEqual(reason.count("line 2"), 1)

    def test_nested_agent_type_does_not_satisfy_outer_call(self):
        self.define_agent("scout", model="haiku", effort="low")
        self.assertDenied(workflow("""
            await agent(build(agent('x', {agentType: 'scout'})))
        """), "missing model, effort in agent(build(")

    def test_outer_options_after_nested_call_still_count(self):
        self.assertSilent(workflow("""
            await agent(build(agent('x', {model: 'a', effort: 'b'})), {model: 'c', effort: 'd'})
        """))

    def test_variable_opts_rejected(self):
        self.assertDenied(workflow("await agent(p, opts)"), "line 1")

    def test_agent_type_definition_counts(self):
        self.define_agent("scout", model="haiku", effort="low")
        self.assertSilent(workflow("await agent('a', {agentType: 'scout'})"))

    def test_agent_type_inside_prompt_text_does_not_count(self):
        self.define_agent("scout", model="haiku", effort="low")
        self.assertDenied(workflow("await agent(\"say agentType: 'scout'\", {schema: S})"), "line 1")

    def test_quoted_agent_type_key(self):
        self.define_agent("scout", model="haiku", effort="low")
        self.assertSilent(workflow('await agent("a", {"agentType": "scout"})'))

    def test_relative_script_path_uses_hook_cwd(self):
        (self.project / "wf.js").write_text("await agent('a')")
        self.assertDenied(pre("Workflow", {"scriptPath": "wf.js"}, cwd=str(self.project)), "line 1")

    def test_agent_type_partial_definition(self):
        self.define_agent("scout", model="haiku")
        self.assertDenied(workflow("await agent('a', {agentType: 'scout'})"), "missing effort")

    def test_script_path(self):
        path = self.project / "wf.js"
        path.write_text("await agent('a')")
        self.assertDenied(pre("Workflow", {"scriptPath": str(path)}), "line 1")

    def test_named_workflow_not_checked(self):
        self.assertSilent(pre("Workflow", {"name": "saved"}))

    def test_missing_script_path_left_to_tool(self):
        self.assertSilent(pre("Workflow", {"scriptPath": str(self.project / "nope.js")}))


class RunnerTests(IsolatedEnv):
    HOOK = pre("Agent", {"prompt": "x"})

    def test_warn_mode_adds_context_without_deny(self):
        out = self.decide(self.HOOK, {"rules": {"subagent_model_effort": {"mode": "warn"}}})
        hso = out["hookSpecificOutput"]
        self.assertNotIn("permissionDecision", hso)
        self.assertIn("subagent_model_effort", hso["additionalContext"])

    def test_off_mode(self):
        self.assertSilent(self.HOOK, {"rules": {"subagent_model_effort": {"mode": "off"}}})

    def test_env_escape_hatch(self):
        with mock.patch.dict(os.environ, {"GUARDRAILS_OFF": "other, subagent_model_effort"}):
            self.assertSilent(self.HOOK)

    def test_never_allows_or_asks(self):
        for hook in (self.HOOK, pre("Agent", {"prompt": "x", "model": "m", "effort": "low"})):
            out = self.decide(hook) or {}
            decision = out.get("hookSpecificOutput", {}).get("permissionDecision")
            self.assertIn(decision, (None, "deny"))

    def crash(self, on_error, mode="enforce"):
        cfg = {"rules": {"subagent_model_effort": {"mode": mode, "on_error": on_error}}}
        with mock.patch.object(sme, "check", side_effect=RuntimeError("boom")):
            return self.decide(self.HOOK, cfg)

    def test_crash_fails_closed(self):
        self.assertIn("crashed", self.crash("closed")["hookSpecificOutput"]["permissionDecisionReason"])

    def test_crash_in_warn_mode_only_warns(self):
        hso = self.crash("closed", mode="warn")["hookSpecificOutput"]
        self.assertNotIn("permissionDecision", hso)
        self.assertIn("crashed", hso["additionalContext"])

    def test_crash_fails_open(self):
        self.assertIsNone(self.crash("open"))

    def test_unknown_rule_fails_closed_without_crashing(self):
        out = self.decide(self.HOOK, {"rules": {"no_such_rule": {"mode": "enforce"}}})
        self.assertIn("no_such_rule crashed", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_logs_decisions(self):
        denials, warnings, records = runner.evaluate({**self.HOOK, "session_id": "s1"}, ENFORCE)
        runner.log({**self.HOOK, "session_id": "s1"}, records)
        entry = json.loads(runner.LOG_PATH.read_text().splitlines()[0])
        self.assertEqual((entry["rule"], entry["action"], entry["session_id"]), ("subagent_model_effort", "deny", "s1"))
        self.assertEqual(entry["permission_mode"], "auto")

    def test_end_to_end_subprocess(self):
        env = {**os.environ, "GUARDRAILS_OFF": "", "GUARDRAILS_LOG": str(runner.LOG_PATH)}
        run = lambda hook: subprocess.run(
            [sys.executable, str(GUARDRAILS / "runner.py")], input=json.dumps(hook),
            capture_output=True, text=True, env=env)
        denied = run(self.HOOK)
        self.assertEqual(denied.returncode, 0, denied.stderr)
        self.assertEqual(json.loads(denied.stdout)["hookSpecificOutput"]["permissionDecision"], "deny")
        ok = run(pre("Agent", {"prompt": "x", "model": "sonnet", "effort": "low"}))
        self.assertEqual((ok.returncode, ok.stdout), (0, ""))


class ConfigConsistencyTests(unittest.TestCase):
    """Catch the easy wiring mistakes: a configured rule that never runs."""

    def setUp(self):
        self.config = runner.load_config()
        settings = json.loads((GUARDRAILS.parent / "settings.json").read_text())
        self.pre_matchers = [
            g.get("matcher", "*") for g in settings["hooks"].get("PreToolUse", [])
            if any("guardrails/runner.py" in h.get("command", "") for h in g["hooks"])
        ]

    def test_every_configured_rule_imports(self):
        import importlib
        for name, opts in self.config["rules"].items():
            rule = importlib.import_module(f"rules.{name}")
            self.assertTrue(callable(rule.check), name)
            self.assertIn(opts.get("mode", "warn"), ("enforce", "warn", "off"), name)
            self.assertIn(opts.get("on_error", "closed"), ("closed", "open"), name)

    def test_settings_matcher_routes_every_rule_tool(self):
        import importlib
        import re
        for name in self.config["rules"]:
            rule = importlib.import_module(f"rules.{name}")
            if "PreToolUse" not in rule.EVENTS:
                continue
            for tool in getattr(rule, "TOOLS", None) or ["*"]:
                routed = any(m == "*" or re.fullmatch(m, tool) for m in self.pre_matchers)
                self.assertTrue(routed, f"{name}: tool {tool} isn't in the PreToolUse matcher")


if __name__ == "__main__":
    unittest.main()
