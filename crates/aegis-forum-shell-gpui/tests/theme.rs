// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The shell paints with the generated Concordia constants and nothing else
//! (M28 E28-5, D05, D101).
//!
//! The conversion tests hold [`theme::color`] to the token's channels. The
//! sweep reads every source file of this crate and refuses a colour literal
//! outside `src/theme.rs`, so a colour typed into a view instead of taken from
//! `ui/concordia-tokens/concordia-tokens.css` fails `make verify-all`.

use std::fs;
use std::path::{Path, PathBuf};

use aegis_concordia::{Rgb, tokens};
use aegis_forum_shell_gpui::theme;
use gpui::Rgba;

/// The colour constructors gpui exports; a call to one outside the theme
/// module is a colour that did not come from the token file.
const COLOUR_CALLS: [&str; 4] = ["rgb(", "rgba(", "hsla(", "hsl("];

/// The source directories the sweep reads; the crate has no deeper tree.
const SOURCE_DIRS: [&str; 2] = ["src", "src/bin"];

/// The one file allowed to build a gpui colour.
const THEME_FILE: &str = "src/theme.rs";

/// More files than this means the sweep is reading the wrong tree.
const MAX_FILES: usize = 64;

fn channels(c: Rgba) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// True when the byte before `at` cannot continue an identifier, or there is
/// none.
fn follows_non_ident(bytes: &[u8], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|before| bytes.get(before))
        .is_none_or(|b| !is_ident_byte(*b))
}

/// True when `line` calls one of [`COLOUR_CALLS`] as a free function or a
/// path (`gpui::rgb(`), not as part of a longer name (`to_rgb(`).
fn calls_colour(line: &str) -> bool {
    COLOUR_CALLS.iter().any(|call| {
        line.match_indices(call)
            .any(|(at, _)| follows_non_ident(line.as_bytes(), at))
    })
}

/// The 1-based numbers of the lines in `source` that build a colour literal.
fn colour_literal_lines(source: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| calls_colour(line))
        .map(|(i, _)| i.saturating_add(1))
        .collect()
}

fn source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for dir in SOURCE_DIRS {
        let entries = fs::read_dir(root.join(dir)).map_err(|e| format!("{dir}: {e}"))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("{dir}: {e}"))?.path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    if files.len() > MAX_FILES {
        return Err(format!(
            "{} source files, more than {MAX_FILES}",
            files.len()
        ));
    }
    Ok(files)
}

#[test]
fn color_carries_the_token_channels() {
    let c = theme::color(Rgb {
        red: 0x20,
        green: 0x40,
        blue: 0x60,
    });
    assert_eq!(channels(c), [32.0 / 255.0, 64.0 / 255.0, 96.0 / 255.0, 1.0]);
}

#[test]
fn color_maps_the_channel_extremes() {
    let black = theme::color(Rgb {
        red: 0,
        green: 0,
        blue: 0,
    });
    let white = theme::color(Rgb {
        red: 255,
        green: 255,
        blue: 255,
    });
    assert_eq!(channels(black), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(channels(white), [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn surface_is_the_surface_token() {
    assert_eq!(
        channels(theme::surface()),
        channels(theme::color(tokens::COLOR_SURFACE))
    );
}

#[test]
fn sweep_refuses_a_planted_literal() {
    let planted = "fn paint() -> Div {\n    div().bg(rgb(0x0020_4060))\n}\n";
    assert_eq!(colour_literal_lines(planted), vec![2]);
    assert_eq!(
        colour_literal_lines("let c = gpui::hsla(0.5, 0.5, 0.5, 1.0);"),
        vec![1]
    );
}

#[test]
fn sweep_passes_names_that_only_contain_a_constructor() {
    let source = "let c = theme::surface();\nlet d = to_rgb(x);\nlet e = srgba(y);\n";
    assert_eq!(colour_literal_lines(source), Vec::<usize>::new());
}

#[test]
fn shell_sources_take_every_colour_from_the_theme() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = source_files(root)?;
    assert!(
        files.len() >= 2,
        "sweep read {} files; expected the crate's sources",
        files.len()
    );
    let mut found = Vec::new();
    for path in files {
        if path.ends_with(THEME_FILE) {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in colour_literal_lines(&text) {
            found.push(format!("{}:{line}", path.display()));
        }
    }
    assert!(
        found.is_empty(),
        "colour literals outside {THEME_FILE}: {found:?}"
    );
    Ok(())
}
