// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// The minimal static build: index.html plus the one component, written to
// dist/. `vite preview` serves dist/ on the loopback interface for the suite;
// the container that runs it has no network, so nothing else is reachable.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  build: { outDir: 'dist', emptyOutDir: true },
  preview: { host: '127.0.0.1', port: 4173, strictPort: true },
});
