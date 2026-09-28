// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Reads the admitted toolchain back from inside the container, as installed
// from the lockfile, and prints one JSON object for tools/verify_a11y.py to
// compare with its pins. With --launch it also starts the pinned Chromium and
// reports the version the browser itself gives. A browser revision the image
// does not carry is an error here: nothing downloads one.
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

// Package manifests are read by path, not through require.resolve: several of
// these packages do not export ./package.json. A direct dependency is linked
// under ./node_modules; a transitive one sits beside its dependent in the
// node_modules directory pnpm's virtual store gives that dependent.
function manifest(name, parent) {
  const base = parent === undefined ? path.resolve('node_modules') : parent.modules;
  const dir = realpathSync(path.join(base, name));
  const version = JSON.parse(readFileSync(path.join(dir, 'package.json'), 'utf8')).version;
  const modules = name.startsWith('@') ? path.dirname(path.dirname(dir)) : path.dirname(dir);
  return { dir, modules, version };
}

const test = manifest('@playwright/test');
const runner = manifest('playwright', test);
const core = manifest('playwright-core', runner);
const browsers = JSON.parse(readFileSync(path.join(core.dir, 'browsers.json'), 'utf8')).browsers;
const shell = browsers.find((entry) => entry.name === 'chromium-headless-shell');
const root = process.env.PLAYWRIGHT_BROWSERS_PATH || '';
const report = {
  node: process.version,
  playwrightTest: test.version,
  playwrightCore: core.version,
  axeCore: manifest('axe-core').version,
  axePlaywright: manifest('@axe-core/playwright').version,
  svelte: manifest('svelte').version,
  vite: manifest('vite').version,
  headlessShell: { revision: shell.revision, browserVersion: shell.browserVersion },
  browsersPath: root,
  revisionPresent: existsSync(path.join(root, `chromium_headless_shell-${shell.revision}`)),
};

if (process.argv.includes('--launch')) {
  const { chromium } = await import(path.join(core.dir, 'index.mjs'));
  const browser = await chromium.launch({ headless: true, timeout: 30_000 });
  report.launched = browser.version();
  await browser.close();
}
process.stdout.write(`${JSON.stringify(report)}\n`);
