// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Planted suppression (D100): an eslint-disable comment does not turn a
// finding off, because the configuration sets noInlineConfig; the eval is
// still reported, and so is the ignored directive.
export function run(source) {
  // eslint-disable-next-line no-eval
  return eval(source);
}
