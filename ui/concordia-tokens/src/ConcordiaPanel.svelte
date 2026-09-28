<!--
  SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
  SPDX-License-Identifier: EUPL-1.2

  The one Svelte 5 component milestone M04 builds: it consumes the Concordia
  base tokens and toggles the .concordia-sr-only state. Every colour, stroke and
  motion value below is a var() of a token, so a theme reaches it by overriding
  the token (D76); tools/test_a11y.py holds this block to that rule too.
-->
<script>
  let showValues = $state(false);
</script>

<main class="concordia-panel">
  <h1 class="concordia-panel__title">Concordia base tokens</h1>
  <p class="concordia-panel__lead">
    This panel draws only from the P12 base tokens. Move through it with the Tab
    key: every stop shows the focus ring.
  </p>

  <form class="concordia-panel__form" onsubmit={(event) => event.preventDefault()}>
    <label class="concordia-panel__label" for="concordia-display-name">Display name</label>
    <input
      id="concordia-display-name"
      class="concordia-panel__input"
      type="text"
      autocomplete="nickname"
    />
  </form>

  <button
    type="button"
    class="concordia-panel__button"
    aria-pressed={showValues}
    aria-controls="concordia-token-values"
    onclick={() => (showValues = !showValues)}
  >
    Show token values
  </button>

  <ul
    id="concordia-token-values"
    class="concordia-panel__values"
    class:concordia-sr-only={!showValues}
  >
    <li>Focus ring width: 3 px, never below 2 px</li>
    <li>Focus ring contrast: at least 3 to 1</li>
    <li>Motion: none while reduced motion is requested</li>
  </ul>

  <p class="concordia-panel__more">
    <a class="concordia-panel__link" href="#concordia-token-values">Read the token values</a>
  </p>
</main>

<style>
  .concordia-panel {
    box-sizing: border-box;
    max-width: var(--concordia-measure);
    margin: 0 auto;
    padding: var(--concordia-space-3);
  }

  .concordia-panel__title {
    margin: 0 0 var(--concordia-space-2);
    font-size: 1.75rem;
    line-height: 1.25;
  }

  .concordia-panel__lead {
    margin: 0 0 var(--concordia-space-3);
    color: var(--concordia-color-text-muted);
  }

  .concordia-panel__form {
    display: flex;
    flex-direction: column;
    gap: var(--concordia-space-1);
    margin: 0 0 var(--concordia-space-3);
  }

  .concordia-panel__input {
    box-sizing: border-box;
    min-height: var(--concordia-target-min);
    padding: 0 var(--concordia-space-1);
    border: var(--concordia-stroke-width) var(--concordia-stroke-style)
      var(--concordia-color-border);
    border-radius: var(--concordia-radius);
    background-color: var(--concordia-color-surface);
    color: var(--concordia-color-text);
    font: inherit;
  }

  .concordia-panel__button {
    min-width: var(--concordia-target-min);
    min-height: var(--concordia-target-min);
    padding: 0 var(--concordia-space-2);
    border: var(--concordia-stroke-width) var(--concordia-stroke-style)
      var(--concordia-color-accent);
    border-radius: var(--concordia-radius);
    background-color: var(--concordia-color-accent);
    color: var(--concordia-color-on-accent);
    font: inherit;
    transition-property: background-color, color;
    transition-duration: var(--concordia-motion-duration);
    transition-timing-function: var(--concordia-motion-easing);
  }

  .concordia-panel__button[aria-pressed='true'] {
    background-color: var(--concordia-color-surface);
    color: var(--concordia-color-accent);
  }

  .concordia-panel__values {
    margin: var(--concordia-space-2) 0;
    padding: var(--concordia-space-2) var(--concordia-space-3);
    background-color: var(--concordia-color-surface-raised);
    color: var(--concordia-color-text);
  }

  .concordia-panel__more {
    margin: var(--concordia-space-2) 0 0;
  }

  .concordia-panel__link {
    display: inline-block;
    min-height: var(--concordia-target-min);
    line-height: var(--concordia-target-min);
    color: var(--concordia-color-accent);
  }
</style>
