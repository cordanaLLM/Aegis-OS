# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Shared test support and test assertions for tools test suite."""

import ast
import contextlib
import io
from pathlib import Path
import tempfile

BRANCHES = (ast.If, ast.For, ast.While, ast.IfExp, ast.ExceptHandler, ast.With, ast.Assert)
EVALUATION_NAMES = {"eval", "exec", "compile"}


def complexity(function):
    """Return a McCabe-style count: one plus each branch and each extra boolean operand."""
    count = 1
    for node in ast.walk(function):
        if isinstance(node, BRANCHES):
            count += 1
        elif isinstance(node, ast.BoolOp):
            count += len(node.values) - 1
        elif isinstance(node, ast.comprehension):
            count += 1 + len(node.ifs)
    return count


def function_index(tree):
    """Return every function definition in `tree` by name."""
    return {node.name: node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)}


def calls(tree, name):
    """Return all Call AST nodes in `tree` calling function `name`."""
    return [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call) and getattr(node.func, "id", None) == name
    ]


def assert_never_evaluates_code(test_case, tree):
    """Assert that `tree` never references eval, exec, or compile."""
    names = {node.id for node in ast.walk(tree) if isinstance(node, ast.Name)}
    test_case.assertFalse(names & EVALUATION_NAMES)


def setup_temp_directory(test_case):
    """Set up self.temp and self.base on `test_case`, registered for cleanup."""
    temp = tempfile.TemporaryDirectory()
    test_case.temp = temp
    test_case.base = Path(temp.name)
    test_case.addCleanup(temp.cleanup)
    return test_case.base


def capture(function, *args, **kwargs):
    """Return (result, stdout) of `function` called with `args` and `kwargs`."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        result = function(*args, **kwargs)
    return result, out.getvalue()


def passing(cases, replace=None, line=""):
    """Return a child report in which every case of `cases` passed."""
    lines = [line if case == replace else f"PASS {case}" for case in cases]
    return "\n".join(lines + ["RESULT pass"]) + "\n"


def assert_complexity_sees_branches(test_case, complexity_fn=complexity):
    """Assert that complexity calculation properly accounts for branching."""
    planted = ast.parse(
        "def f(a, b):\n    if a and b:\n        return 1\n    for x in a:\n        pass\n"
    )
    test_case.assertEqual(complexity_fn(planted.body[0]), 4)


def assert_only_run_starts_a_process(test_case, functions, expected=None):
    """Assert only the designated runner starts a subprocess."""
    target = [("run", "Popen")] if expected is None else expected
    starters = [
        (function.name, node.attr)
        for function in functions.values()
        for node in ast.walk(function)
        if isinstance(node, ast.Attribute)
        and getattr(node.value, "id", "") == "subprocess"
        and node.attr in {"Popen", "run", "call", "check_output", "check_call"}
    ]
    test_case.assertEqual(starters, target)


def assert_every_wait_is_bounded(test_case, tree):
    """Assert that every wait and communicate call carries a timeout."""
    waits = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call) and getattr(node.func, "attr", "") in {"wait", "communicate"}
    ]
    test_case.assertTrue(waits)
    for node in waits:
        test_case.assertIn("timeout", {keyword.arg for keyword in node.keywords}, node.lineno)


def assert_only_run_starts_a_process_and_every_wait_is_bounded(
    test_case, functions, tree, expected_starters=None
):
    """Assert only run starts a subprocess and every wait/communicate is bounded."""
    assert_only_run_starts_a_process(test_case, functions, expected=expected_starters)
    assert_every_wait_is_bounded(test_case, tree)


def assert_function_limits(
    test_case, functions, max_lines, max_complexity, max_statements, complexity_fn=complexity
):
    """Assert no function exceeds length, statement or complexity thresholds."""
    offenders = []
    for node in functions.values():
        length = node.end_lineno - node.lineno + 1
        statements = sum(1 for inner in ast.walk(node) if isinstance(inner, ast.stmt)) - 1
        if length > max_lines or complexity_fn(node) > max_complexity:
            offenders.append(f"{node.name}: {length} lines, complexity {complexity_fn(node)}")
        elif statements > max_statements:
            offenders.append(f"{node.name}: {statements} statements")
    test_case.assertEqual(offenders, [])
