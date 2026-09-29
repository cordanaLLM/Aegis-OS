// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// Planted HISS-08 violations (D100): each form of dynamic execution once.
export function run(source) {
  const direct = eval(source);
  const later = setTimeout('globalThis.planted = true', 1);
  const built = new Function('return 1');
  return [direct, later, built];
}
