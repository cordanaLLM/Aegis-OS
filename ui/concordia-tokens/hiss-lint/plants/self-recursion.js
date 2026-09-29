// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Planted HISS-01 violations (D100): four ways a function calls itself by
// name, each reported once by aegis-hiss/no-self-recursion.
export function factorial(n) {
  return n <= 1 ? 1 : n * factorial(n - 1);
}

export const countdown = (n) => (n <= 0 ? 0 : countdown(n - 1));

export function viaClosure(items) {
  return items.map((item) => (Array.isArray(item) ? viaClosure(item) : item));
}

export class Walker {
  walk(node) {
    return node.next === null ? node : this.walk(node.next);
  }
}
