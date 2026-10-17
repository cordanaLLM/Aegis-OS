#!/usr/bin/env python3
"""Refuse a call cycle between Rust functions anywhere in the tree (HISS-01).

The pinned Praetor engine decides direct self-recursion by a bare name only. It
lists mutual, nested, qualified, trait-impl, macro-template and macro-input
recursion as known misses (cordanaLLM/praetor#1320), so a planted pair of
mutually recursive functions passes ``clippy -D warnings`` and
``praetorctl audit``. This check closes that gap in this repository until the
engine fix is adopted at a later pin (issue #219); it then retires.

It lexes every tracked or untracked, non-ignored ``*.rs`` file, records every
function item (free, nested, inherent, trait-impl, trait default and
``macro_rules`` template functions) and resolves each call it can name:

- a bare call reaches a function nested in the caller or one of its enclosing
  functions, then a free function of the caller's module, never a method;
- ``self.f()`` and ``Self::f()`` reach the inherent method of the impl's type
  first, then the type's only trait-impl method of that name, or in a trait
  default method the trait's own function;
- a path call reaches a free function by ``crate::``, ``self::``, ``super::``
  or child-module path, or an associated function by ``Type::f``;
- calls inside a macro's input count, as the macro may only wrap its input.

A method call on any receiver other than ``self`` is delegation and adds no
edge, and a bare call adds none while the caller binds that name as a value (a
parameter, a ``let``, a pattern or a closure parameter), so every recorded edge
is a call rustc would make to that function. A cycle in the resulting call
graph, a function calling itself included, fails the check.

Usage: ``python3 tools/verify_recursion.py [root]``.
``tools/test_verify_recursion.py`` plants each recursion form the engine misses
and each form it must accept.
"""

import bisect
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GIT_TIMEOUT = 60
# Scalar bounds on the token walks below (HISS-02).
MAX_PATH_SEGMENTS = 64
MAX_NESTING = 4096

KEYWORDS = frozenset(
    "as async await break const continue crate dyn else enum extern false fn for if impl in "
    "let loop match mod move mut pub ref return self Self static struct super trait true "
    "type unsafe use where while yield".split()
)
OPEN = {"(": ")", "[": "]", "{": "}"}
CLOSE = frozenset(OPEN.values())
TARGET_DIRS = frozenset({"tests", "benches", "examples"})

TOKEN = re.compile(
    r"""
    (?P<space>\s+)
  | (?P<line>//[^\n]*)
  | (?P<block>/\*)
  | (?P<raw>(?:b|c)?r(?P<hashes>\#*)")
  | (?P<string>(?:b|c)?"(?:[^"\\]|\\.)*")
  | (?P<char>b?'(?:[^'\\\n]|\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]*\}|.))')
  | r\#(?P<rawid>[^\W\d]\w*)
  | (?P<life>'[^\W\d]\w*)
  | (?P<id>[^\W\d]\w*)
  | (?P<num>\d\w*(?:\.\d\w*)*)
  | (?P<punct>::|->|=>|.)
    """,
    re.VERBOSE | re.DOTALL,
)


class RecursionCheckError(Exception):
    """A Rust source cannot be read as the check expects."""


class Token:
    """One lexed token: its text, kind and source line."""

    __slots__ = ("text", "kind", "line")

    def __init__(self, text, kind, line):
        self.text = text
        self.kind = kind
        self.line = line


class Fn:
    """One function item and the calls its body makes."""

    __slots__ = ("key", "name", "file", "line", "target", "module", "owner", "calls", "values")

    def __init__(self, key, name, site, owner):
        self.key = key
        self.name = name
        self.file, self.line, self.target, self.module = site
        self.owner = owner
        self.calls = []
        self.values = set()


def skip_block_comment(text, start):
    """Return the offset after the nested block comment opening at ``start``."""
    depth = 0
    for match in re.compile(r"/\*|\*/").finditer(text, start):
        depth += 1 if match.group() == "/*" else -1
        if depth == 0:
            return match.end()
    raise RecursionCheckError(f"offset {start}: unterminated block comment")


def skip_raw_string(text, match):
    """Return the offset after the raw string whose opening ``match`` names."""
    end = text.find('"' + match.group("hashes"), match.end())
    if end < 0:
        raise RecursionCheckError(f"offset {match.start()}: unterminated raw string")
    return end + 1 + len(match.group("hashes"))


def lex(text):
    """Return the tokens of ``text`` with comments and literal contents removed."""
    newlines = [m.start() for m in re.finditer("\n", text)]
    tokens = []
    pos = 0
    for _ in range(len(text) + 1):
        if pos >= len(text):
            return tokens
        match = TOKEN.match(text, pos)
        kind = match.lastgroup if match.lastgroup != "hashes" else "raw"
        end = match.end()
        if kind == "block":
            end = skip_block_comment(text, pos)
        elif kind == "raw":
            end = skip_raw_string(text, match)
        if kind not in ("space", "line", "block"):
            word = match.group("rawid") or match.group()
            kind = "id" if kind in ("id", "rawid") else kind
            tokens.append(Token(word, kind, bisect.bisect_left(newlines, pos) + 1))
        pos = end
    return tokens


def match_brackets(tokens):
    """Map each opening bracket's index to its closing bracket's index."""
    pairs = {}
    stack = []
    for index, token in enumerate(tokens):
        if token.kind != "punct":
            continue
        if token.text in OPEN:
            stack.append(index)
        elif token.text in CLOSE:
            if not stack or OPEN[tokens[stack[-1]].text] != token.text:
                raise RecursionCheckError(f"line {token.line}: unbalanced {token.text!r}")
            pairs[stack.pop()] = index
    if stack:
        raise RecursionCheckError(f"line {tokens[stack[-1]].line}: unclosed bracket")
    return pairs


def module_parts(parts):
    """Return the module path a file at ``parts`` below its source root names."""
    names = list(parts)
    names[-1] = names[-1][: -len(".rs")]
    if names[-1] == "mod" or len(names) == 1 and names[-1] in ("lib", "main"):
        names.pop()
    return tuple(names)


def place(crate, parts):
    """Return the compilation target and module path of a file inside ``crate``."""
    if parts[0] == "src" and len(parts) > 2 and parts[1] == "bin":
        name = parts[2][: -len(".rs")] if len(parts) == 3 else parts[2]
        return (crate, "bin", name), module_parts(parts[3:]) if len(parts) > 3 else ()
    if parts[0] == "src" and len(parts) > 1:
        return (crate, "src"), module_parts(parts[1:])
    if parts[0] in TARGET_DIRS and len(parts) > 2:
        return (crate, parts[0], parts[1]), module_parts(parts[2:])
    return (crate,) + tuple(parts), ()


def locate(root, path):
    """Return the target and module path of ``path``; a workspace-level file is its own target."""
    rel = path.relative_to(root)
    for depth in range(len(rel.parts) - 1, 0, -1):
        crate = pathlib.PurePosixPath(*rel.parts[:depth])
        if (root / crate / "Cargo.toml").is_file():
            return place(crate.as_posix(), rel.parts[depth:])
    return (rel.as_posix(),), ()


class Graph:
    """Every function the scanned files define and the indexes calls resolve against."""

    def __init__(self):
        self.fns = []
        self.modules = set()
        self.free = {}
        self.nested = {}
        self.inherent = {}
        self.trait_impl = {}
        self.trait_fns = {}

    def register(self, fn):
        """Record ``fn`` under the index its owner selects."""
        self.fns.append(fn)
        kind = fn.owner[0]
        if kind == "fn":
            index, scope = self.nested, fn.owner[1].key
        elif kind == "impl":
            index = self.inherent if fn.owner[2] is None else self.trait_impl
            scope = (fn.target, fn.owner[1])
        elif kind == "trait":
            index, scope = self.trait_fns, (fn.target, fn.owner[1])
        else:
            index, scope = self.free, (fn.target, fn.module)
        if fn.name is not None:
            index.setdefault(scope, {}).setdefault(fn.name, []).append(fn)


def last_ident(tokens, start, end, pairs):
    """Return the last identifier outside angle brackets in ``tokens[start:end]``."""
    name = None
    angle = 0
    index = start
    for _ in range(end - start + 1):
        if index >= end:
            break
        text = tokens[index].text
        angle += {"<": 1, ">": -1}.get(text, 0)
        if text == "where" and angle == 0:
            break
        if angle == 0 and tokens[index].kind == "id" and text not in KEYWORDS:
            name = text
        index = pairs.get(index, index) + 1
    return name


class Scanner:
    """Walk one file's tokens and record its functions and their calls."""

    def __init__(self, graph, tokens, site):
        self.graph = graph
        self.tokens = tokens
        self.pairs = match_brackets(tokens)
        self.file, self.target, self.module = site
        self.frames = []

    def kind(self, index):
        """Return the kind of the token at ``index``, or None past either end."""
        if 0 <= index < len(self.tokens):
            return self.tokens[index].kind
        return None

    def text(self, index):
        """Return the text of the token at ``index``, or None past either end."""
        if 0 <= index < len(self.tokens):
            return self.tokens[index].text
        return None

    def scan(self):
        """Visit every token once, items first, calls in function bodies after."""
        index = 0
        for _ in range(len(self.tokens) + 1):
            if index >= len(self.tokens):
                return
            while self.frames and self.frames[-1][0] < index:
                self.frames.pop()
            index = self.visit(index)

    def visit(self, index):
        """Handle the token at ``index`` and return the next index to visit."""
        token = self.tokens[index]
        handler = {
            "mod": self.mod_item,
            "impl": self.impl_item,
            "trait": self.trait_item,
            "fn": self.fn_item,
        }.get(token.text if token.kind == "id" else None)
        following = handler(index) if handler else None
        if following is not None:
            return following
        if token.kind == "id" and token.text not in KEYWORDS:
            self.note_use(index)
        return index + 1

    def push(self, end, frame):
        """Open a scope that closes at token ``end``."""
        if len(self.frames) >= MAX_NESTING:
            raise RecursionCheckError(f"{self.file}: items nested deeper than {MAX_NESTING}")
        self.frames.append((end, frame))

    def module_path(self):
        """Return the module path of the innermost scope."""
        inline = tuple(frame[1] for _, frame in self.frames if frame[0] == "mod")
        return self.module + inline

    def owner(self):
        """Return the innermost function, impl, trait or module scope."""
        for _, frame in reversed(self.frames):
            if frame[0] in ("fn", "impl", "trait"):
                return frame
        return ("module",)

    def mod_item(self, index):
        """Open an inline module."""
        if self.kind(index + 1) != "id" or self.text(index + 2) != "{":
            return None
        name = self.tokens[index + 1].text
        self.push(self.pairs[index + 2], ("mod", name))
        self.graph.modules.add((self.target, self.module_path()))
        return index + 3

    def header_end(self, start):
        """Return the index of the body brace after ``start``, or of the token ending the item."""
        angle = 0
        index = start
        for _ in range(len(self.tokens) + 1):
            text = self.text(index)
            if text is None or text in (";", "}") and angle == 0 or self.fragment_body(index):
                return index
            if text == "{" and angle == 0:
                return index
            angle += {"<": 1, ">": -1}.get(text, 0)
            index = self.pairs.get(index, index) + 1 if text in OPEN else index + 1
        return index

    def fragment_body(self, index):
        """Return True when a ``$name`` at ``index`` stands where a body opens.

        In a ``macro_rules`` template ``fn f(..) -> u32 $body`` takes its body
        from the fragment, while ``fn f(n: $t) -> $t { .. }`` only names a type.
        A metavariable after a finished type or parameter list is the body.
        """
        if self.text(index) != "$":
            return False
        before = self.tokens[index - 1]
        return before.text in (")", "]", ">") or before.kind == "id" and before.text not in KEYWORDS

    def impl_item(self, index):
        """Open an impl block and record its type and trait."""
        body = self.header_end(index + 1)
        if self.text(body) != "{":
            return None
        start = self.header_end_generics(index + 1) if self.text(index + 1) == "<" else index + 1
        split = self.trait_split(start, body)
        trait = None if split is None else last_ident(self.tokens, start, split, self.pairs)
        typ = last_ident(self.tokens, start if split is None else split + 1, body, self.pairs)
        self.push(self.pairs[body], ("impl", typ or "<unnamed>", trait))
        return body + 1

    def header_end_generics(self, start):
        """Return the index after the generic list opening at ``start``."""
        angle = 0
        index = start
        for _ in range(len(self.tokens) + 1):
            angle += {"<": 1, ">": -1}.get(self.text(index), 0)
            index = self.pairs.get(index, index) + 1
            if angle == 0 or index >= len(self.tokens):
                return index
        return index

    def trait_split(self, start, end):
        """Return the index of the ``for`` that separates trait and type, if any."""
        angle = 0
        index = start
        for _ in range(end - start + 1):
            if index >= end:
                return None
            text = self.text(index)
            angle += {"<": 1, ">": -1}.get(text, 0)
            if text == "for" and angle == 0 and self.text(index + 1) != "<":
                return index
            if text == "where" and angle == 0:
                return None
            index = self.pairs.get(index, index) + 1
        return None

    def trait_item(self, index):
        """Open a trait block."""
        if self.kind(index + 1) != "id":
            return None
        body = self.header_end(index + 2)
        if self.text(body) != "{":
            return None
        self.push(self.pairs[body], ("trait", self.tokens[index + 1].text))
        return body + 1

    def fn_item(self, index):
        """Record a function item and open its body."""
        named = self.kind(index + 1) == "id"
        if not named and not (self.text(index + 1) == "$" and self.kind(index + 2) == "id"):
            return None
        after = index + 2 if named else index + 3
        params = self.header_end_generics(after) if self.text(after) == "<" else after
        if self.text(params) != "(":
            return None
        name = self.tokens[index + 1].text if named else None
        site = (self.file, self.tokens[index].line, self.target, self.module_path())
        fn = Fn((self.file, index), name, site, self.owner())
        fn.values.update(t.text for t in self.tokens[params : self.pairs[params]] if t.kind == "id")
        self.graph.register(fn)
        body = self.header_end(self.pairs[params] + 1)
        if self.text(body) != "{":
            return body
        self.push(self.pairs[body], ("fn", fn))
        return body + 1

    def current_fn(self):
        """Return the innermost function whose body holds the current token."""
        for _, frame in reversed(self.frames):
            if frame[0] == "fn":
                return frame[1]
            if frame[0] in ("impl", "trait", "mod"):
                return None
        return None

    def call_open(self, index):
        """Return True when the identifier at ``index`` is called, turbofish or not."""
        after = index + 1
        if self.text(after) == "::" and self.text(after + 1) == "<":
            after = self.header_end_generics(after + 1)
        return self.text(after) == "("

    def note_use(self, index):
        """Record a call, or a value use of a name, inside the current function."""
        fn = self.current_fn()
        if fn is None:
            return
        name = self.tokens[index].text
        before = self.text(index - 1)
        if self.call_open(index):
            self.note_call(fn, index, before)
        elif before not in (".", "::") and self.text(index + 1) not in ("::", "!"):
            fn.values.add(name)

    def note_call(self, fn, index, before):
        """Record the call the identifier at ``index`` makes, in the form its spelling names."""
        name = self.tokens[index].text
        line = self.tokens[index].line
        if before == ".":
            if self.text(index - 2) == "self" and self.text(index - 3) not in (".", "::"):
                fn.calls.append(("self", (), name, line))
        elif before == "::":
            segments = self.path_before(index)
            if segments == ("Self",):
                fn.calls.append(("self", (), name, line))
            elif segments:
                fn.calls.append(("path", segments, name, line))
        else:
            fn.calls.append(("bare", (), name, line))

    def path_before(self, index):
        """Return the path segments before the name at ``index``, or None for ``<T as U>::f``."""
        segments = []
        cursor = index - 1
        for _ in range(MAX_PATH_SEGMENTS):
            if self.text(cursor) != "::":
                return tuple(segments)
            prior = cursor - 1
            if self.text(prior) == ">":
                prior = self.angle_open(prior) - 1
                if self.text(prior) != "::":
                    return None
                prior -= 1
            if prior < 0 or self.tokens[prior].kind != "id":
                return None
            segments.insert(0, self.tokens[prior].text)
            cursor = prior - 1
        return None

    def angle_open(self, close):
        """Return the index of the ``<`` that the ``>`` at ``close`` ends."""
        angle = 0
        for index in range(close, -1, -1):
            angle += {">": 1, "<": -1}.get(self.text(index), 0)
            if angle == 0:
                return index
        return 0


def method_targets(graph, fn, name):
    """Return the functions ``self.name()`` or ``Self::name()`` reach from ``fn``."""
    kind = fn.owner[0]
    if kind == "trait":
        return graph.trait_fns.get((fn.target, fn.owner[1]), {}).get(name, [])
    if kind != "impl":
        return []
    return type_targets(graph, fn.target, fn.owner[1], name)


def type_targets(graph, target, typ, name):
    """Return the inherent method, else the only trait-impl method, of ``typ`` named ``name``."""
    inherent = graph.inherent.get((target, typ), {}).get(name, [])
    if inherent:
        return inherent
    implemented = graph.trait_impl.get((target, typ), {}).get(name, [])
    return implemented if len(implemented) == 1 else []


def bare_targets(graph, fn, name):
    """Return the nested or free functions a bare call to ``name`` reaches from ``fn``."""
    if name in fn.values:
        return []
    scope = fn
    for _ in range(MAX_NESTING):
        found = graph.nested.get(scope.key, {}).get(name)
        if found:
            return found
        if scope.owner[0] != "fn":
            break
        scope = scope.owner[1]
    return graph.free.get((fn.target, fn.module), {}).get(name, [])


def path_base(graph, fn, segments):
    """Return the module path the leading segments name and the segments left over."""
    head = segments[0]
    if head == "crate":
        return (), segments[1:]
    if head == "self":
        return fn.module, segments[1:]
    if head == "super":
        module = fn.module
        rest = segments
        for _ in range(MAX_PATH_SEGMENTS):
            if not rest or rest[0] != "super":
                break
            module, rest = module[:-1], rest[1:]
        return module, rest
    if (fn.target, fn.module + (head,)) in graph.modules:
        return fn.module, segments
    return None, segments


def path_targets(graph, fn, segments, name):
    """Return the functions a path call reaches from ``fn``."""
    base, rest = path_base(graph, fn, segments)
    if base is not None:
        found = graph.free.get((fn.target, base + tuple(rest)), {}).get(name)
        if found:
            return found
    if rest and rest[-1][:1].isupper():
        return type_targets(graph, fn.target, rest[-1], name)
    return []


def resolve(graph):
    """Return the call edges between recorded functions."""
    edges = {}
    for fn in graph.fns:
        for form, segments, name, _ in fn.calls:
            if form == "bare":
                found = bare_targets(graph, fn, name)
            elif form == "self":
                found = method_targets(graph, fn, name)
            else:
                found = path_targets(graph, fn, segments, name)
            edges.setdefault(fn.key, set()).update(callee.key for callee in found)
    return edges


def strongly_connected(keys, edges):
    """Return the strongly connected components of the call graph (iterative Tarjan)."""
    index, low, on_stack, stack, components = {}, {}, set(), [], []
    for root in keys:
        if root in index:
            continue
        work = [(root, iter(sorted(edges.get(root, ()))))]
        index[root] = low[root] = len(index)
        stack.append(root)
        on_stack.add(root)
        for _ in range(4 * len(keys) * (len(keys) + 1)):
            if not work:
                break
            visit_step(work, edges, index, low, on_stack, stack, components)
    return components


def visit_step(work, edges, index, low, on_stack, stack, components):
    """Advance the Tarjan walk by one edge or one finished node."""
    node, children = work[-1]
    child = next(children, None)
    if child is None:
        work.pop()
        if work:
            parent = work[-1][0]
            low[parent] = min(low[parent], low[node])
        if low[node] == index[node]:
            component = []
            for _ in range(len(stack)):
                member = stack.pop()
                on_stack.discard(member)
                component.append(member)
                if member == node:
                    break
            components.append(component)
    elif child not in index:
        index[child] = low[child] = len(index)
        stack.append(child)
        on_stack.add(child)
        work.append((child, iter(sorted(edges.get(child, ())))))
    elif child in on_stack:
        low[node] = min(low[node], index[child])


def cycles(graph, edges):
    """Return each call cycle as a sorted list of functions."""
    by_key = {fn.key: fn for fn in graph.fns}
    found = []
    for component in strongly_connected(sorted(by_key), edges):
        if len(component) > 1 or component[0] in edges.get(component[0], ()):
            found.append(sorted((by_key[key] for key in component), key=lambda f: f.key))
    return found


def rust_files(root):
    """Return every tracked or untracked, non-ignored ``*.rs`` file below ``root``."""
    listed = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.rs"],
        cwd=root,
        capture_output=True,
        check=True,
        timeout=GIT_TIMEOUT,
        env={key: value for key, value in os.environ.items() if not key.startswith("GIT_")},
    ).stdout.decode("utf-8")
    return sorted({root / name for name in listed.split("\0") if name})


def check(root, files):
    """Return the graph and every call cycle among the functions in ``files``."""
    graph = Graph()
    for path in files:
        if not path.is_file():
            continue
        target, module = locate(root, path)
        name = path.relative_to(root).as_posix()
        graph.modules.add((target, module))
        try:
            tokens = lex(path.read_text(encoding="utf-8"))
            Scanner(graph, tokens, (name, target, module)).scan()
        except RecursionCheckError as error:
            raise RecursionCheckError(f"{name}: {error}") from error
    edges = resolve(graph)
    return graph, edges, cycles(graph, edges)


def describe(fn):
    """Return a function's name and location for a report line."""
    return f"{fn.name or '<template>'} ({fn.file}:{fn.line})"


def main(argv):
    """Run the check and report PASS or each cycle."""
    root = pathlib.Path(argv[1]).resolve() if len(argv) > 1 else ROOT
    try:
        files = rust_files(root)
        graph, edges, found = check(root, files)
    except (OSError, UnicodeDecodeError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1
    except RecursionCheckError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    for cycle in found:
        names = " -> ".join(describe(fn) for fn in cycle + cycle[:1])
        print(f"FAIL: HISS-01 call cycle: {names}", file=sys.stderr)
    if found:
        return 1
    count = sum(len(targets) for targets in edges.values())
    print(
        f"PASS: {len(files)} Rust files, {len(graph.fns)} functions, {count} resolved call edges,"
        " no call cycle (HISS-01)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
