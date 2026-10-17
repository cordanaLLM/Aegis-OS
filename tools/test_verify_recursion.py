"""Planted cases for tools/verify_recursion.py (issue #219).

The refused cases are the six Rust recursion forms the pinned engine lists as
known misses (cordanaLLM/praetor#1320, ``testdata/HISS-01/rust/gap``) plus the
cross-file and method forms the same call graph decides. The accepted cases are
the engine's delegation, forwarding and shadowing negatives and the lexer
boundaries (comments, strings, characters, lifetimes) a token walk must not read
as code. One case plants a mutually recursive pair into a copy of a real crate.
"""

import contextlib
import io
import pathlib
import shutil
import sys
import tempfile
import textwrap
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import verify_recursion as vr  # noqa: E402

REPO = vr.ROOT

REFUSED = {
    "mutual": """
        pub fn is_even(n: u32) -> bool {
            if n == 0 { return true; }
            is_odd(n - 1)
        }
        pub fn is_odd(n: u32) -> bool {
            if n == 0 { return false; }
            is_even(n - 1)
        }
    """,
    "three_cycle": """
        fn a(n: u32) -> u32 { b(n) }
        fn b(n: u32) -> u32 { c(n) }
        fn c(n: u32) -> u32 { if n == 0 { 0 } else { a(n - 1) } }
    """,
    "nested": """
        pub fn outer(n: u32) -> u32 {
            fn inner(n: u32) -> u32 {
                if n == 0 { return 0; }
                inner(n - 1)
            }
            inner(n)
        }
    """,
    "turbofish": """
        pub fn sum<T: Copy + Into<u64>>(items: &[T]) -> u64 {
            match items.split_first() {
                Some((head, rest)) => (*head).into() + sum::<T>(rest),
                None => 0,
            }
        }
    """,
    "crate_path": """
        pub fn depth(n: u32) -> u32 {
            if n == 0 { return 0; }
            1 + crate::depth(n - 1)
        }
    """,
    "super_path": """
        pub fn ping(n: u32) -> u32 { if n == 0 { 0 } else { inner::pong(n - 1) } }
        pub mod inner {
            pub fn pong(n: u32) -> u32 { super::ping(n) }
        }
    """,
    "trait_impl_self": """
        pub struct Countdown(u32);
        impl Iterator for Countdown {
            type Item = u32;
            fn next(&mut self) -> Option<u32> {
                if self.0 == 0 { return None; }
                self.0 -= 1;
                self.next()
            }
        }
    """,
    "trait_default": """
        pub trait Walk {
            fn step(&self) -> u32;
            fn walk(&self, n: u32) -> u32 { if n == 0 { 0 } else { self.walk(n - 1) } }
        }
    """,
    "macro_template": """
        macro_rules! countdown {
            ($t:ty) => {
                pub fn countdown(n: $t) -> $t {
                    if n == 0 { 0 } else { countdown(n - 1) }
                }
            };
        }
        countdown!(u32);
    """,
    "macro_input": """
        pub fn parse_expr(p: &mut Parser) -> Result<Expr, Error> {
            if p.eat('(') {
                let inner = ok!(parse_expr(p));
                return Ok(Expr::Group(Box::new(inner)));
            }
            Ok(Expr::Atom(p.next()))
        }
    """,
    "associated_self": """
        pub struct Tree;
        impl Tree {
            pub fn count(n: u32) -> u32 { if n == 0 { 0 } else { 1 + Self::count(n - 1) } }
        }
    """,
    "type_path_mutual": """
        pub struct Tree;
        impl Tree {
            pub fn left(n: u32) -> u32 { if n == 0 { 0 } else { Tree::right(n - 1) } }
            pub fn right(n: u32) -> u32 { Tree::left(n) }
        }
    """,
    "method_mutual": """
        pub struct Counter { left: u32 }
        impl Counter {
            pub fn tick(&mut self) -> u32 { if self.left == 0 { 0 } else { self.tock() } }
            fn tock(&mut self) -> u32 { self.left -= 1; self.tick() }
        }
    """,
    "impl_trait_return": """
        pub fn make(n: u32) -> impl Fn() -> u32 {
            if n == 0 { return move || 0; }
            make(n - 1)
        }
    """,
    "direct": """
        pub fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }
    """,
}

ACCEPTED = {
    "delegation": """
        use std::io;
        pub struct Wrapper<W> { inner: W }
        impl<W: io::Write> Wrapper<W> {
            pub fn flush(&mut self) -> io::Result<()> { self.inner.flush() }
        }
        mod codec {
            pub fn parse(s: &str) -> usize { s.len() }
        }
        pub fn parse(s: &str) -> usize { codec::parse(s) }
    """,
    "trait_impl_forwarding": """
        pub trait Ones { fn count_ones(&self) -> u32; }
        pub struct W(u32);
        impl W {
            pub fn count_ones(&self) -> u32 { self.0.count_ones() }
        }
        impl Ones for W {
            fn count_ones(&self) -> u32 { self.count_ones() }
        }
        pub struct A;
        pub struct B;
        pub struct E;
        impl From<B> for E { fn from(_: B) -> Self { E } }
        impl From<A> for E { fn from(_a: A) -> Self { Self::from(B) } }
    """,
    "bare_call_in_impl": """
        pub fn render(v: u32) -> String { v.to_string() }
        pub struct View(u32);
        impl View {
            pub fn render(&self) -> String { render(self.0) }
            pub fn build(v: u32) -> String { render(v) }
        }
    """,
    "macro_template_body": """
        macro_rules! dispatch {
            ($name:ident, $x:ident, $body:block) => {
                pub fn $name(n: u32) -> u32 {
                    unsafe fn fn_impl($x: u32) -> u32 $body
                    unsafe { if n > 1 { fn_impl(n) } else { fn_impl(0) } }
                }
            };
        }
        dispatch!(run, n, { n + 1 });
    """,
    "shadowed": """
        pub fn step(n: u32) -> u32 { let step = |x: u32| x + 1; step(n) }
        pub fn apply(apply: fn(u32) -> u32, n: u32) -> u32 { apply(n) }
        pub fn each(handlers: &[fn()]) { for each in handlers { each(); } }
        pub fn first(slot: Option<fn() -> u8>) -> u8 {
            match slot { Some(first) if first() > 0 => first(), _ => 0 }
        }
        pub struct S { pub pick: fn() -> u8 }
        pub fn pick(s: S) -> u8 { let S { pick } = s; pick() }
    """,
    "same_name_other_module": """
        pub fn load(n: u32) -> u32 { store::load(n) }
        pub mod store {
            pub fn load(n: u32) -> u32 { n + 1 }
        }
    """,
    "fn_pointer_types": """
        pub fn run(f: fn(u32) -> u32, g: impl Fn() -> u8) -> u32 { f(u32::from(g())) }
        pub fn twice(n: u32) -> u32 { run(|x| x + n, || 1) }
    """,
    "lexer_boundaries": """
        // pub fn hidden() { hidden() }
        /* nested /* comment */ fn also_hidden() { also_hidden() } */
        pub fn quoted<'a>(s: &'a str) -> &'a str {
            let _ = "quoted(s) } {";
            let _ = r#"quoted(s) " } "#;
            let _ = b"quoted(s)";
            let _ = ['{', '}', '\\'', '\\u{7d}'];
            s
        }
    """,
    "where_clause_impl": """
        pub trait Size { fn size(&self) -> u32; }
        pub struct Wrap<T>(T);
        impl<T> Wrap<T> { pub fn size(&self) -> u32 { 1 } }
        impl<T> Size for Wrap<T> where T: Size { fn size(&self) -> u32 { self.size() } }
    """,
    "unrelated_receiver": """
        pub struct List(Vec<u32>);
        impl List {
            pub fn len(&self) -> usize { self.0.len() }
            pub fn first(&self) -> Option<u32> { self.0.first().copied() }
        }
    """,
}


def write_crate(root, files):
    """Write a member crate holding ``files`` (path to source) below ``root``.

    The crate sits below the root as workspace members do: a file whose nearest
    manifest is the root's is its own target, as the shared test helpers are.
    """
    crate = root / "crates" / "planted"
    crate.mkdir(parents=True)
    (crate / "Cargo.toml").write_text('[package]\nname = "planted"\n', encoding="utf-8")
    for name, source in files.items():
        path = crate / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(source), encoding="utf-8")
    return [crate / name for name in files]


class Planted(unittest.TestCase):
    """Each planted source is checked alone in a temporary crate."""

    def run_check(self, files):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            paths = write_crate(root, files)
            _, _, found = vr.check(root, paths)
        return [sorted(fn.name or "<template>" for fn in cycle) for cycle in found]

    def test_refuses_each_recursion_form(self):
        for name, source in REFUSED.items():
            with self.subTest(name):
                self.assertTrue(self.run_check({"src/lib.rs": source}), name)

    def test_accepts_each_non_recursive_form(self):
        for name, source in ACCEPTED.items():
            with self.subTest(name):
                self.assertEqual(self.run_check({"src/lib.rs": source}), [], name)

    def test_mutual_pair_names_both_functions(self):
        self.assertEqual(self.run_check({"src/lib.rs": REFUSED["mutual"]}), [["is_even", "is_odd"]])

    def test_refuses_cycle_across_module_files(self):
        files = {
            "src/lib.rs": "pub mod a;\npub mod b;\n",
            "src/a.rs": "pub fn f(n: u32) -> u32 { if n == 0 { 0 } else { crate::b::g(n) } }\n",
            "src/b/mod.rs": "pub fn g(n: u32) -> u32 { super::a::f(n - 1) }\n",
        }
        self.assertEqual(self.run_check(files), [["f", "g"]])

    def test_targets_do_not_share_free_functions(self):
        files = {
            "src/lib.rs": "pub fn tick(n: u32) -> u32 { tock(n) }\n",
            "src/bin/tool.rs": "fn tock(n: u32) -> u32 { n }\nfn tick(n: u32) -> u32 { n }\n",
            "tests/cli.rs": "fn tock(n: u32) -> u32 { tick(n) }\nfn tick(n: u32) -> u32 { n }\n",
        }
        self.assertEqual(self.run_check(files), [])

    def test_refuses_unbalanced_source(self):
        with self.assertRaisesRegex(
            vr.RecursionCheckError, "crates/planted/src/lib.rs: line 1: unbalanced"
        ):
            self.run_check({"src/lib.rs": "pub fn broken() { ) }\n"})

    def test_refuses_unterminated_comment(self):
        with self.assertRaisesRegex(vr.RecursionCheckError, "unterminated block comment"):
            self.run_check({"src/lib.rs": "/* open\npub fn f() {}\n"})

    def test_empty_source_has_no_function(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            graph, edges, found = vr.check(root, write_crate(root, {"src/lib.rs": ""}))
        self.assertEqual((graph.fns, found), ([], []))


class RealCrate(unittest.TestCase):
    """A planted pair in a copy of a committed crate is refused; the copy alone passes."""

    CRATE = "crates/aegis-justitia"

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        shutil.copytree(REPO / self.CRATE, self.root / self.CRATE)

    def tearDown(self):
        self.tmp.cleanup()

    def files(self):
        return sorted((self.root / self.CRATE).rglob("*.rs"))

    def test_committed_crate_passes(self):
        _, edges, found = vr.check(self.root, self.files())
        self.assertEqual(found, [])
        self.assertTrue(edges)

    def test_planted_pair_is_refused(self):
        lib = self.root / self.CRATE / "src/lib.rs"
        planted = "\nfn planted_a(n: u32) -> u32 { planted_b(n) }\n"
        planted += "fn planted_b(n: u32) -> u32 { if n == 0 { 0 } else { planted_a(n - 1) } }\n"
        lib.write_text(lib.read_text(encoding="utf-8") + planted, encoding="utf-8")
        _, _, found = vr.check(self.root, self.files())
        self.assertEqual(
            [sorted(fn.name for fn in cycle) for cycle in found], [["planted_a", "planted_b"]]
        )


class Main(unittest.TestCase):
    """The command reports PASS on the repository and exits 1 on a planted cycle."""

    def run_main(self, root):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = vr.main(["verify_recursion.py", str(root)])
        return code, out.getvalue(), err.getvalue()

    def test_repository_passes(self):
        code, out, _ = self.run_main(REPO)
        self.assertEqual(code, 0)
        self.assertIn("no call cycle", out)

    def test_planted_cycle_fails_with_both_names(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp).resolve()
            paths = write_crate(root, {"src/lib.rs": REFUSED["mutual"]})
            with mock.patch.object(vr, "rust_files", return_value=paths):
                code, _, err = self.run_main(root)
        self.assertEqual(code, 1)
        self.assertIn("HISS-01 call cycle: is_even (crates/planted/src/lib.rs:2) -> is_odd", err)

    def test_unreadable_source_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp).resolve()
            paths = write_crate(root, {"src/lib.rs": "pub fn f() { ]\n"})
            with mock.patch.object(vr, "rust_files", return_value=paths):
                code, _, err = self.run_main(root)
        self.assertEqual(code, 1)
        self.assertIn("FAIL: crates/planted/src/lib.rs: line 1: unbalanced", err)


if __name__ == "__main__":
    unittest.main()
