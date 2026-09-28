// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Mounts the one component on the static page the accessibility suite loads.
import '../concordia-tokens.css';
import { mount } from 'svelte';
import ConcordiaPanel from './ConcordiaPanel.svelte';

const target = document.getElementById('app');
if (target === null) {
  throw new Error('the page has no #app element to mount the panel into');
}
mount(ConcordiaPanel, { target });
