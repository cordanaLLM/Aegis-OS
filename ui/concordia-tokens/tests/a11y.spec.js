// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// The M04 accessibility suite. axe-core runs the D81 tag set with no impact
// filter; the focus-indicator, D16 boundary, contrast boundary and text-scaling
// checks are measured from computed style by tests/probe.js, because axe-core
// 4.13.0 has no rule for any of them. Titles that say "browser emulation" or
// "browser accessibility tree" are exactly that: Playwright's media emulation
// and Chromium's tree, never the desktop portal or AT-SPI2 (REQ-P12-02 and
// REQ-P12-03 keep those halves for M16 and M27).
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {
  CLAUSES,
  D81_TAGS,
  STUB_THEME,
  criterionOf,
  focusButton,
  openPanel,
  record,
  setTokens,
  tabStops,
} from './harness.js';

const BUCKETS = ['passes', 'violations', 'incomplete', 'inapplicable'];

function executedRules(results) {
  const executed = {};
  for (const bucket of BUCKETS) {
    for (const rule of results[bucket]) {
      executed[rule.id] = { bucket, tags: rule.tags, nodes: rule.nodes.length };
    }
  }
  return executed;
}

// One row per success criterion an executed rule is tagged with: its V4.1.1
// and V3.2.1 clauses from the clause map, and the rules that reached it.
function criterionRows(executed) {
  const rows = {};
  const unmapped = [];
  for (const [id, rule] of Object.entries(executed)) {
    for (const criterion of rule.tags.map(criterionOf).filter(Boolean)) {
      const clause = CLAUSES.criteria[criterion];
      if (clause === undefined) {
        unmapped.push(`${id}: ${criterion}`);
        continue;
      }
      rows[criterion] ??= { ...clause, evaluated: [], inapplicable: [] };
      rows[criterion][rule.bucket === 'inapplicable' ? 'inapplicable' : 'evaluated'].push(id);
    }
  }
  return { rows, unmapped };
}

async function axeRulesFor(page, criterion) {
  const tag = `wcag${criterion.replaceAll('.', '')}`;
  return page.evaluate((wanted) => window.axe.getRules([wanted]).map((rule) => rule.ruleId), tag);
}

test('default state: zero axe-core violations under the D81 tag set, no impact filter', async ({ page, browser }) => {
  await openPanel(page);
  const results = await new AxeBuilder({ page }).withTags(D81_TAGS).analyze();
  const executed = executedRules(results);
  const { rows, unmapped } = criterionRows(executed);
  const notCovered = {};
  for (const criterion of CLAUSES['not-covered-by-axe']) {
    notCovered[criterion] = { ...CLAUSES.criteria[criterion], rules: await axeRulesFor(page, criterion) };
  }
  record('default-state', {
    axe: results.testEngine.version,
    browser: browser.version(),
    runOnly: results.toolOptions.runOnly,
    counts: Object.fromEntries(BUCKETS.map((bucket) => [bucket, results[bucket].length])),
    violations: results.violations.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length })),
    incomplete: results.incomplete.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length })),
    targetSize: executed['target-size'] ?? null,
    criteria: rows,
    unmapped,
    notCovered,
  });
  expect(results.toolOptions.runOnly).toEqual({ type: 'tag', values: D81_TAGS });
  expect(executed['target-size'], 'target-size must be among the executed rules').toBeDefined();
  expect(executed['target-size'].bucket, 'target-size must evaluate the panel targets').toBe('passes');
  expect(results.violations.map((v) => `${v.id} (${v.impact})`)).toEqual([]);
  expect(results.incomplete.map((v) => `${v.id} (${v.impact})`)).toEqual([]);
  expect(unmapped).toEqual([]);
  for (const [criterion, entry] of Object.entries(notCovered)) {
    expect(entry.rules, `axe-core evaluates ${criterion} after all`).toEqual([]);
  }
});

test('focus indicator: every tab stop meets the D16 floor and 3:1, measured from computed style', async ({ page }) => {
  await openPanel(page);
  const stops = await tabStops(page);
  record('focus-stops', { stops });
  expect(stops.map((stop) => stop.element.split(' ')[0])).toEqual([
    'input#concordia-display-name',
    'button',
    'a',
  ]);
  for (const stop of stops) {
    expect(stop.reasons, stop.element).toEqual([]);
    expect(stop.kind, stop.element).toBe('outline');
    expect(stop.width, stop.element).toBe(3);
  }
});

test('falsifier: a stripped outline fails the measured check while axe-core still reports nothing', async ({ page }) => {
  await openPanel(page);
  const before = await focusButton(page);
  await page.addStyleTag({ content: ':focus-visible { outline: none !important; }' });
  const after = await page.evaluate(() => window.__concordiaProbe.measureActive());
  const axe = await new AxeBuilder({ page }).withTags(D81_TAGS).analyze();
  record('falsifier-stripped-outline', {
    before,
    after,
    axeViolationsWithTheOutlineStripped: axe.violations.map((v) => v.id),
  });
  expect(before.pass).toBe(true);
  expect(after.kind).toBe('none');
  expect(after.pass).toBe(false);
  expect(axe.violations.map((v) => v.id)).toEqual([]);
});

test('D16 width boundary: the 3px token and the 2px floor pass, 1px fails', async ({ page }) => {
  await openPanel(page);
  const rows = [];
  for (const width of ['3px', '2px', '1px']) {
    await setTokens(page, { '--concordia-focus-ring-width': width });
    const stop = await focusButton(page);
    rows.push({ token: width, measured: stop.width, pass: stop.pass, reasons: stop.reasons });
  }
  record('boundary-width', { rows });
  expect(rows.map((row) => [row.measured, row.pass])).toEqual([
    [3, true],
    [2, true],
    [1, false],
  ]);
});

test('contrast boundary: a 3:1 ring passes and a 2.99:1 ring fails', async ({ page }) => {
  await openPanel(page);
  const rows = [];
  for (const ring of ['#fa34ff', '#b278ff']) {
    await setTokens(page, { '--concordia-color-surface': '#ffffff', '--concordia-color-focus-ring': ring });
    const stop = await focusButton(page);
    rows.push({ ring, ratio: stop.ratio, pass: stop.pass, reasons: stop.reasons });
  }
  const thresholds = await page.evaluate(() =>
    [3, 2.9999, 2.99].map((ratio) => ({ ratio, pass: window.__concordiaProbe.verdict(2, ratio).pass })),
  );
  record('boundary-contrast', { rows, thresholds });
  expect(rows[0].ratio).toBeGreaterThanOrEqual(3);
  expect(rows[0].ratio).toBeLessThan(3.0001);
  expect(rows[1].ratio).toBeLessThanOrEqual(2.99);
  expect(rows[1].ratio).toBeGreaterThan(2.989);
  expect(rows.map((row) => row.pass)).toEqual([true, false]);
  expect(thresholds.map((row) => row.pass)).toEqual([true, false, false]);
});

test('D76 stub: a theme that lowers the ring below the 2px floor fails the boundary test', async ({ page }) => {
  await openPanel(page);
  await page.addStyleTag({ path: STUB_THEME });
  const stop = await focusButton(page);
  record('d76-stub-theme', { fixture: 'tests/fixtures/stub-heads-up-theme.css', stop });
  expect(stop.width).toBe(1);
  expect(stop.pass).toBe(false);
  expect(stop.reasons.join(' ')).toContain('below the D16 floor');
});

test('D76 tokens: one override per token reaches the component with no second token source', async ({ page }) => {
  await openPanel(page);
  await setTokens(page, {
    '--concordia-color-accent': '#123456',
    '--concordia-color-surface': '#fefefe',
    '--concordia-stroke-width': '4px',
    '--concordia-motion-duration': '321ms',
    '--concordia-focus-ring-width': '5px',
    '--concordia-color-focus-ring': '#654321',
  });
  const stop = await focusButton(page);
  // The link carries the accent with no transition, so its colour is read at
  // once; the button's own background would still be mid-transition.
  const seen = await page.evaluate(() => {
    const button = getComputedStyle(document.querySelector('.concordia-panel__button'));
    return {
      linkColor: getComputedStyle(document.querySelector('.concordia-panel__link')).color,
      buttonBorder: button.borderTopWidth,
      buttonTransition: button.transitionDuration,
      bodyBackground: getComputedStyle(document.body).backgroundColor,
    };
  });
  record('d76-token-overrides', { seen, stop });
  expect(seen).toEqual({
    linkColor: 'rgb(18, 52, 86)',
    buttonBorder: '4px',
    buttonTransition: '0.321s',
    bodyBackground: 'rgb(254, 254, 254)',
  });
  expect([stop.width, stop.color]).toEqual([5, [101, 67, 33, 1]]);
});

test('200% text scaling: no overflow and no clipped text, and the detector sees a planted clip', async ({ page }) => {
  await openPanel(page);
  const probe = () => page.evaluate(() => window.__concordiaProbe.overflow());
  const base = await probe();
  await page.evaluate(() => {
    document.documentElement.style.fontSize = '200%';
  });
  const fontSize = await page.evaluate(() => getComputedStyle(document.querySelector('.concordia-panel__lead')).fontSize);
  const scaled = await probe();
  await page.evaluate(() => {
    const box = document.createElement('div');
    box.style.cssText = 'width: 120px; overflow: hidden; white-space: nowrap';
    box.textContent = 'a fixed-width box that clips its text at double size';
    document.querySelector('main').append(box);
  });
  const planted = await probe();
  record('text-scaling-200', { base, fontSize, scaled, planted });
  expect(base).toEqual([]);
  expect(fontSize).toBe('32px');
  expect(scaled).toEqual([]);
  expect(planted.length).toBeGreaterThan(0);
});

test('[browser emulation, not portal evidence] REQ-P12-02 CSS half: reduced motion zeroes the motion token', async ({ page }) => {
  await openPanel(page);
  const read = () =>
    page.evaluate(() => getComputedStyle(document.querySelector('.concordia-panel__button')).transitionDuration);
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  const normal = await read();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const reduced = await read();
  record('emulation-reduced-motion', {
    source: 'Playwright page.emulateMedia; no org.freedesktop.portal.Settings value was read',
    normal,
    reduced,
  });
  expect(normal).toBe('0.15s');
  expect(reduced).toBe('0s');
});

test('[browser emulation, not portal evidence] forced colours: the ring stays at the token width and axe-core finds nothing', async ({ page }) => {
  await openPanel(page);
  await page.emulateMedia({ forcedColors: 'active' });
  const stops = await tabStops(page);
  const results = await new AxeBuilder({ page }).withTags(D81_TAGS).analyze();
  record('emulation-forced-colors', {
    source: 'Playwright page.emulateMedia forcedColors; no desktop contrast setting was read',
    stops,
    violations: results.violations.map((v) => v.id),
  });
  expect(stops).toHaveLength(3);
  for (const stop of stops) {
    expect(stop.reasons, stop.element).toEqual([]);
  }
  expect(results.violations.map((v) => v.id)).toEqual([]);
});

test('[browser accessibility tree, not AT-SPI2] REQ-P12-03 CSS half: the hidden values stay exposed', async ({ page }) => {
  await openPanel(page);
  const main = page.getByRole('main');
  const collapsed = await main.ariaSnapshot();
  await page.getByRole('button', { name: 'Show token values' }).click();
  const expanded = await main.ariaSnapshot();
  const hiddenClass = await page.evaluate(() => document.getElementById('concordia-token-values').className);
  record('accessibility-tree', {
    source: "Chromium's accessibility tree through Playwright; no AT-SPI2 bus was queried",
    collapsed,
    expanded,
    hiddenClass,
  });
  for (const snapshot of [collapsed, expanded]) {
    expect(snapshot).toContain('heading "Concordia base tokens" [level=1]');
    expect(snapshot).toContain('textbox "Display name"');
    expect(snapshot).toContain('link "Read the token values"');
    expect(snapshot).toContain('listitem: "Focus ring width: 3 px, never below 2 px"');
  }
  expect(collapsed).toContain('button "Show token values"');
  expect(expanded).toContain('button "Show token values" [pressed]');
  expect(hiddenClass).not.toContain('concordia-sr-only');
});
