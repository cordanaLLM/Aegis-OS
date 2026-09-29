// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// In-page measurement for the checks axe-core cannot make. axe-core 4.13.0 has
// no rule for the width or the contrast of a focus indicator and none for text
// resized to 200%, so a suite that asked axe alone would report zero
// violations for a page with no focus ring at all. This script reads the
// focused element's computed style instead: the outline, or a box-shadow ring
// when there is no outline, its width in CSS pixels and its WCAG 2.2 contrast
// against the colour it is drawn on.
//
// It is injected as a classic script and exposes one object,
// window.__concordiaProbe; the suite calls it through page.evaluate. Every
// loop is bounded (HISS-02). The declarations sit in a strict-mode block rather
// than an immediately invoked function, so each stays block-scoped without a
// 170-line wrapper function that the D100 lint would count against HISS-04.
'use strict';

{
  // D16: the 3px token may go no lower than 2px; WCAG 2.2 SC 1.4.11 and 2.4.13
  // put the indicator's contrast floor at 3:1. Both are inclusive.
  const FOCUS_FLOOR_PX = 2;
  const CONTRAST_FLOOR = 3;
  const MAX_ANCESTORS = 64;
  const MAX_ELEMENTS = 2000;
  const CANVAS = [255, 255, 255, 1];

  function parseColor(text) {
    const match = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/.exec(
      String(text).trim(),
    );
    if (match === null) {
      return null;
    }
    let alpha = match[4] === undefined ? 1 : Number.parseFloat(match[4]);
    if (String(match[4]).endsWith('%')) {
      alpha /= 100;
    }
    return [Number(match[1]), Number(match[2]), Number(match[3]), alpha];
  }

  function channel(value) {
    const c = value / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }

  function luminance(rgb) {
    return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
  }

  function contrastRatio(a, b) {
    const la = luminance(a);
    const lb = luminance(b);
    return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
  }

  function blend(top, bottom) {
    const alpha = top[3];
    return [0, 1, 2].map((i) => top[i] * alpha + bottom[i] * (1 - alpha)).concat([1]);
  }

  // The colour a ring drawn outside `element` sits on: the parent's painted
  // background, composited up the tree until an opaque one is found.
  function backgroundBehind(element) {
    const layers = [];
    let node = element.parentElement;
    for (let depth = 0; node !== null && depth < MAX_ANCESTORS; depth += 1) {
      const colour = parseColor(getComputedStyle(node).backgroundColor);
      if (colour !== null && colour[3] > 0) {
        layers.push(colour);
        if (colour[3] >= 1) {
          break;
        }
      }
      node = node.parentElement;
    }
    return layers.reverse().reduce((under, layer) => blend(layer, under), CANVAS);
  }

  function shadowRing(text) {
    const match = /^(rgba?\([^)]*\))\s+(-?[\d.]+)px\s+(-?[\d.]+)px\s+([\d.]+)px\s+([\d.]+)px$/.exec(
      String(text).split(/,(?![^(]*\))/)[0].trim(),
    );
    if (match === null || Number(match[4]) !== 0) {
      return null;
    }
    return { width: Number(match[5]), color: parseColor(match[1]) };
  }

  function indicator(element) {
    const style = getComputedStyle(element);
    const width = Number.parseFloat(style.outlineWidth) || 0;
    if (style.outlineStyle !== 'none' && width > 0) {
      return { kind: 'outline', width, color: parseColor(style.outlineColor) };
    }
    const ring = shadowRing(style.boxShadow);
    if (ring !== null && ring.width > 0) {
      return { kind: 'box-shadow', width: ring.width, color: ring.color };
    }
    return { kind: 'none', width: 0, color: null };
  }

  function verdict(width, ratio) {
    const reasons = [];
    if (!(width >= FOCUS_FLOOR_PX)) {
      reasons.push(`indicator ${width}px is below the D16 floor of ${FOCUS_FLOOR_PX}px`);
    }
    if (!(ratio >= CONTRAST_FLOOR)) {
      reasons.push(`indicator contrast ${ratio.toFixed(4)}:1 is below ${CONTRAST_FLOOR}:1`);
    }
    return { pass: reasons.length === 0, reasons };
  }

  function describe(element) {
    const id = element.id ? `#${element.id}` : '';
    const name = (element.getAttribute('aria-label') || element.textContent || '').trim();
    return `${element.tagName.toLowerCase()}${id} "${name.slice(0, 40)}"`;
  }

  function measure(element) {
    const found = indicator(element);
    const background = backgroundBehind(element);
    const ratio = found.color === null ? 0 : contrastRatio(blend(found.color, background), background);
    return {
      element: describe(element),
      kind: found.kind,
      width: found.width,
      color: found.color,
      background,
      ratio,
      ...verdict(found.width, ratio),
    };
  }

  function measureActive() {
    const element = document.activeElement;
    if (element === null || element === document.body) {
      return { element: 'none', kind: 'none', width: 0, ratio: 0, pass: false, reasons: ['nothing is focused'] };
    }
    return measure(element);
  }

  function hidden(element) {
    return element.closest('.concordia-sr-only') !== null;
  }

  function clipped(element) {
    const style = getComputedStyle(element);
    const x = style.overflowX !== 'visible' && element.scrollWidth > element.clientWidth + 1;
    const y = style.overflowY !== 'visible' && element.scrollHeight > element.clientHeight + 1;
    return x || y;
  }

  // Loss of content under text scaling: the page scrolls sideways, an element
  // reaches past the viewport, or a box clips the text it holds.
  function overflow() {
    const offenders = [];
    const viewport = document.documentElement.clientWidth;
    if (document.documentElement.scrollWidth > viewport + 1) {
      offenders.push(`the page is ${document.documentElement.scrollWidth}px wide in a ${viewport}px viewport`);
    }
    const elements = Array.from(document.body.querySelectorAll('*')).slice(0, MAX_ELEMENTS);
    for (const element of elements) {
      if (hidden(element)) {
        continue;
      }
      if (element.getBoundingClientRect().right > viewport + 1) {
        offenders.push(`${describe(element)} reaches past the viewport`);
      } else if (clipped(element)) {
        offenders.push(`${describe(element)} clips its content`);
      }
    }
    return offenders;
  }

  window.__concordiaProbe = {
    FOCUS_FLOOR_PX,
    CONTRAST_FLOOR,
    contrastRatio,
    parseColor,
    verdict,
    measure,
    measureActive,
    overflow,
  };
}
