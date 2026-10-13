// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
import type { PraetorFigure } from '../../tools/figures/types.ts';

export default {
  title: 'The contract pair gate',
  alt: 'One networked fetch fills a cache; the offline gate checks its identity, then feeds the payloads to imago and nucleus.',
  evidence: [
    'tools/verify_contract_pair.py:fetch_main',
    'tools/verify_contract_pair.py:gate_main',
    'tools/verify_contract_pair.py:skip_reasons',
    'tools/verify_contract_pair.py:run_cases',
    'tools/verify_contract_pair.py:product_accepted_case',
    'tools/verify_contract_pair.py:nucleus_cases',
    'tools/verify_contract_pair.py:retain',
    'Makefile:verify_contract_pair',
  ],
  props: {
    layout: {
      direction: 'row',
      children: [
        {
          direction: 'column',
          gap: 40,
          children: [
            { id: 'pin', label: 'producers.pin.json', sub: 'commits, bounds, digests', shape: 'store' },
            { id: 'fetch', label: 'make contract-fetch', sub: 'the one networked step' },
            { id: 'cache', label: 'Contract cache', sub: 'checkouts, binary, identity', shape: 'store' },
          ],
        },
        { id: 'gate', label: 'make verify-contract', sub: 'offline' },
        { id: 'identity', label: 'Identity cases pass?', shape: 'decision' },
        {
          direction: 'column',
          children: [
            { id: 'imago', label: 'imago binary', sub: 'decodes, refuses' },
            { id: 'nucleus', label: 'nucleus verifier', sub: 'kernel requirement' },
          ],
        },
        { id: 'runs', label: 'Run record', sub: 'argv, exit, output, reports', shape: 'store' },
      ],
    },
    edges: [
      { from: 'pin', to: 'fetch', label: 'pinned commits' },
      { from: 'fetch', to: 'cache', label: 'clone, build' },
      { from: 'pin', to: 'gate', label: 'pin' },
      { from: 'cache', to: 'gate', label: 'cache' },
      { from: 'gate', to: 'identity' },
      { from: 'identity', to: 'imago', label: 'payloads' },
      { from: 'identity', to: 'nucleus', label: 'requirement' },
      { from: 'imago', to: 'runs' },
      { from: 'nucleus', to: 'runs' },
    ],
    steps: [
      {
        label: 'Fetch',
        caption: 'The only step that reaches the network.',
        flow: [
          { edges: 'pin->fetch', say: 'The fetch reads both producers and their commits from the pin.' },
          { edges: 'fetch->cache', say: 'It clones imago and nucleus at those commits, builds imago, and writes the identity record last.' },
        ],
      },
      {
        label: 'Gate',
        caption: 'Nothing a producer prints counts until its identity checks out.',
        flow: [
          { edges: ['pin->gate', 'cache->gate'], say: 'The gate reads the pin and the cache, never the network.' },
          { edges: 'gate->identity', say: 'Five identity cases run first: pin, identity record, both checkouts and the binary provenance.' },
          { edges: ['identity->imago', 'identity->nucleus'], say: 'Then imago decodes or refuses each payload, and nucleus verifies the kernel requirement.' },
          { edges: ['imago->runs', 'nucleus->runs'], say: 'Every invocation, payload and report is retained under the run id.' },
        ],
      },
      {
        label: 'Skip or fail',
        caption: 'An exit 0 is evidence only with case lines above it.',
        flow: [
          { edges: 'cache->gate', say: 'Without go, git or a cached piece, the gate prints SKIP with its reason and exits 0.' },
          { edges: 'gate->identity', say: 'A cache fetched for another pin, or a binary that is not the pinned build, fails and names make contract-fetch.' },
        ],
      },
    ],
  },
} satisfies PraetorFigure;
