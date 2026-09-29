<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: EUPL-1.2

Planted HISS-01 and HISS-08 violations inside a Svelte component's script
(D100): proves .svelte files are parsed and linted, not skipped.
-->
<script>
  let { depth = 3, source = '1' } = $props();

  function nest(level) {
    return level <= 0 ? [] : [nest(level - 1)];
  }

  const tree = $derived(nest(depth));
  const value = $derived(eval(source));
</script>

<p>{tree.length} {value}</p>
