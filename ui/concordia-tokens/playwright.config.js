// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Headless Chromium against the static build that `vite preview` serves on the
// loopback interface. No retries and no allowed failures: one failing
// assertion fails the run (E04-1). The browser is the one the pinned image
// ships under PLAYWRIGHT_BROWSERS_PATH; with the container's network disabled
// there is nothing to download a different one from.
import { defineConfig } from '@playwright/test';

const out = process.env.AEGIS_A11Y_OUT || 'test-output';

export default defineConfig({
  testDir: './tests',
  outputDir: `${out}/test-results`,
  forbidOnly: true,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  reporter: [['list'], ['json', { outputFile: `${out}/results.json` }]],
  use: {
    baseURL: 'http://127.0.0.1:4173',
    browserName: 'chromium',
    headless: true,
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
    colorScheme: 'light',
    reducedMotion: 'no-preference',
    forcedColors: 'none',
    trace: 'off',
    screenshot: 'off',
    video: 'off',
  },
  webServer: {
    // vite directly, not through pnpm exec: Playwright ends the server by its
    // process group, and a server started one launcher further down outlived
    // it and held the run open.
    command: 'node node_modules/vite/bin/vite.js preview',
    url: 'http://127.0.0.1:4173/',
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
