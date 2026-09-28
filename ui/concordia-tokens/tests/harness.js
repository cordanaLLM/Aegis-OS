// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Shared pieces of the M04 suite: the D81 tag set, the clause map, the planted
// mutations the gate's negative cases select, and the per-test report files
// tools/verify_a11y.py reads back. Nothing here filters a result.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));

// D81: exactly these tags, no impact filter, no best-practice tag. The tag set
// alone makes axe-core 4.13.0 run target-size, which it ships disabled.
export const D81_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

export const CLAUSES = JSON.parse(readFileSync(path.join(HERE, 'en301549-clauses.json'), 'utf8'));
export const PROBE = path.join(HERE, 'probe.js');
export const STUB_THEME = path.join(HERE, 'fixtures', 'stub-heads-up-theme.css');

// A 1x1 transparent PNG; planted without alt text it is exactly one image-alt
// violation and nothing else.
const PIXEL =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

// Each mutation the gate plants to prove a failure is a failure. An unknown
// name is refused, so a typo cannot turn a negative case into a silent pass.
const PLANTS = {
  none: async () => {},
  'missing-alt': async (page) => {
    await page.evaluate((src) => {
      const img = document.createElement('img');
      img.src = src;
      img.width = 32;
      img.height = 32;
      document.querySelector('main').append(img);
    }, PIXEL);
  },
  'strip-outline': async (page) => {
    await page.addStyleTag({ content: ':focus-visible { outline: none !important; }' });
  },
  'stub-theme': async (page) => {
    await page.addStyleTag({ path: STUB_THEME });
  },
};

export function plantName() {
  const name = process.env.AEGIS_A11Y_PLANT || 'none';
  if (!Object.hasOwn(PLANTS, name)) {
    throw new Error(`unknown AEGIS_A11Y_PLANT ${JSON.stringify(name)}`);
  }
  return name;
}

export async function openPanel(page) {
  await page.goto('/');
  await page.getByRole('heading', { level: 1, name: 'Concordia base tokens' }).waitFor();
  await page.addScriptTag({ path: PROBE });
  await PLANTS[plantName()](page);
}

// wcag1410 -> 1.4.10, wcag258 -> 2.5.8: principle and guideline are one digit.
export function criterionOf(tag) {
  const match = /^wcag(\d)(\d)(\d{1,2})$/.exec(tag);
  return match === null ? null : `${match[1]}.${match[2]}.${match[3]}`;
}

export function record(name, body) {
  const out = process.env.AEGIS_A11Y_OUT;
  if (!out) {
    return;
  }
  mkdirSync(out, { recursive: true });
  writeFileSync(path.join(out, `${name}.json`), `${JSON.stringify({ plant: plantName(), ...body }, null, 2)}\n`);
}

// Move the sequential focus starting point back to the top of the page. A
// blur() alone leaves it on the element that had focus, so the next Tab would
// continue from there; focusing the body once, through a temporary tabindex,
// resets it.
async function restartTabOrder(page) {
  await page.evaluate(() => {
    document.body.tabIndex = -1;
    document.body.focus();
    document.body.removeAttribute('tabindex');
  });
}

// Tab from the top of the page and measure each stop, until focus returns to
// a stop already seen or leaves the page. Bounded: the panel has three stops.
export async function tabStops(page, limit = 12) {
  await restartTabOrder(page);
  const stops = [];
  const seen = new Set();
  for (let i = 0; i < limit; i += 1) {
    await page.keyboard.press('Tab');
    const stop = await page.evaluate(() => window.__concordiaProbe.measureActive());
    if (stop.element === 'none' || seen.has(stop.element)) {
      break;
    }
    seen.add(stop.element);
    stops.push(stop);
  }
  return stops;
}

export async function setTokens(page, tokens) {
  await page.evaluate((values) => {
    for (const [name, value] of Object.entries(values)) {
      document.documentElement.style.setProperty(name, value);
    }
  }, tokens);
}

// Focus reaches the button by keyboard, so :focus-visible applies exactly as
// it does for a user, rather than through a scripted focus() call.
export async function focusButton(page) {
  const stops = await tabStops(page);
  const stop = stops.find((entry) => entry.element.startsWith('button'));
  if (stop === undefined) {
    throw new Error(`no tab stop is the button: ${JSON.stringify(stops.map((s) => s.element))}`);
  }
  await restartTabOrder(page);
  for (let i = 0; i < stops.indexOf(stop) + 1; i += 1) {
    await page.keyboard.press('Tab');
  }
  return page.evaluate(() => window.__concordiaProbe.measureActive());
}
