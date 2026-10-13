#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the strict audit wrapper.

`judge` is fed audit text of known shape; `run_audit` and `main` run the
Python interpreter as a stand-in command, so every case runs on each platform
the Platform Neutrality matrix covers.
"""

import contextlib
import io
import sys
import unittest

import audit_strict

CLEAN = "\n".join(
    (
        "[PASS] Manifest verified: cordanaLLM/Aegis-OS (Version 1)",
        "       Profiles: [os-image]",
        "[INFO] HISS file scope: 557 files read",
        "[REPORT] HISS-04 complexity measured, not enforced",
        "[SKIP] packer: not applicable, no *.pkr.hcl",
        "[UNSCANNED] No HISS rule examined lua (2 files)",
        "",
        "Audit Summary: configured governance gates passed for cordanaLLM/Aegis-OS.",
    )
)


def with_line(line):
    """Return the clean audit text with line inserted before the summary."""
    head, _, summary = CLEAN.rpartition("\n")
    return f"{head}\n{line}\n{summary}"


class Judge(unittest.TestCase):
    def test_a_clean_audit_passes(self):
        self.assertEqual(audit_strict.judge(0, CLEAN), [])

    def test_a_warning_fails(self):
        problems = audit_strict.judge(0, with_line("[WARN] Workflow triggers (HISS-18): ci.yml:57"))
        self.assertEqual(len(problems), 1)
        self.assertIn("[WARN]", problems[0])

    def test_an_indented_warning_fails(self):
        self.assertTrue(audit_strict.judge(0, with_line("   [WARN] indented")))

    def test_a_failure_behind_the_error_prefix_fails(self):
        self.assertTrue(audit_strict.judge(0, with_line("Error: [FAIL] ruleset differs")))

    def test_a_marker_this_file_does_not_name_fails(self):
        problems = audit_strict.judge(0, with_line("[NOTICE] a marker a later engine adds"))
        self.assertIn("[NOTICE]", problems[0])

    def test_a_marker_inside_a_line_is_text(self):
        self.assertEqual(audit_strict.judge(0, with_line("Profiles: [WARN] in prose")), [])

    def test_a_lower_case_bracket_is_text(self):
        self.assertEqual(audit_strict.judge(0, with_line("[os-image] listed")), [])

    def test_a_nonzero_exit_fails_a_clean_text(self):
        self.assertEqual(audit_strict.judge(1, CLEAN), ["the audit exited 1"])

    def test_a_run_without_its_summary_fails(self):
        head, _, _ = CLEAN.rpartition("\n")
        problems = audit_strict.judge(0, head)
        self.assertEqual(len(problems), 1)
        self.assertIn("summary", problems[0])

    def test_empty_output_fails(self):
        self.assertTrue(audit_strict.judge(0, ""))


def python_command(source):
    """Return a command that runs source in this interpreter, isolated."""
    return [sys.executable, "-I", "-c", source]


class RunAudit(unittest.TestCase):
    def test_combined_output_and_exit_code_are_returned(self):
        source = "import sys; print('out'); print('err', file=sys.stderr); sys.exit(3)"
        with contextlib.redirect_stdout(io.StringIO()) as echoed:
            code, text = audit_strict.run_audit(python_command(source))
        self.assertEqual(code, 3)
        self.assertIn("out", text)
        self.assertIn("err", text)
        self.assertEqual(echoed.getvalue(), text)

    def test_a_command_past_its_deadline_fails(self):
        code, text = audit_strict.run_audit(python_command("import time; time.sleep(30)"), timeout=1)
        self.assertEqual(code, 124)
        self.assertTrue(audit_strict.judge(code, text))

    def test_a_missing_command_fails(self):
        code, text = audit_strict.run_audit(["aegis-no-such-praetorctl"])
        self.assertEqual(code, 127)
        self.assertTrue(audit_strict.judge(code, text))


class Main(unittest.TestCase):
    def run_main(self, argv):
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            with contextlib.redirect_stderr(io.StringIO()):
                code = audit_strict.main(argv)
        return code, printed.getvalue()

    def test_no_command_is_a_usage_error(self):
        self.assertEqual(self.run_main(["audit_strict.py"])[0], 2)

    def test_a_clean_audit_passes_through_main(self):
        source = f"print({CLEAN!r})"
        code, printed = self.run_main(["audit_strict.py", sys.executable, "-I", "-c", source])
        self.assertEqual(code, 0, printed)
        self.assertIn("PASS: the audit printed no warning", printed)

    def test_a_warning_fails_through_main(self):
        source = f"print({with_line('[WARN] planted')!r})"
        code, printed = self.run_main(["audit_strict.py", sys.executable, "-I", "-c", source])
        self.assertEqual(code, 1)
        self.assertIn("carries [WARN]", printed)


if __name__ == "__main__":
    unittest.main()
