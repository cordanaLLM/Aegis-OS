#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
# shellcheck disable=SC2015  # A && ok || fail: echo cannot fail, so C never runs after a true A
# Stub test for e25-1-vfio-run.sh, run by tools/test_e25_vfio_card.py: a fake sysfs tree and a fake sudo that
# applies driver bind effects. Never touches /sys or /dev.
set -euo pipefail
S=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/e25-1-vfio-run.sh
STUBS=()
fails=0
mk() {
  R=$(mktemp -d "${TMPDIR:-/tmp}/e25stub.XXXXXX")
  STUBS+=("$R")
  local p=$R/sys/bus/pci
  mkdir -p "$p/drivers/xe" "$p/drivers/vfio-pci" "$p/drivers/nvidia" "$R/sys/kernel/iommu_groups/917/devices" "$R/dev/vfio" "$R/dev/dri" "$R/evid"
  local d=$p/devices/0000:f3:00.0 n=$p/devices/0000:f6:00.0
  mkdir -p "$d/drm/card0/card0-DP-4" "$d/drm/card0/card0-Writeback-1" "$d/drm/renderD129" "$n"
  echo 0x8086 > "$d/vendor"; echo 0x56a5 > "$d/device"; : > "$d/driver_override"
  echo 0x10de > "$n/vendor"; echo 0x2684 > "$n/device"; echo 0x030000 > "$n/class"
  echo disconnected > "$d/drm/card0/card0-DP-4/status"; echo unknown > "$d/drm/card0/card0-Writeback-1/status"
  ln -s "$p/drivers/xe" "$d/driver"; ln -s "$p/drivers/nvidia" "$n/driver"
  ln -s "$R/sys/kernel/iommu_groups/917" "$d/iommu_group"
  ln -s "$d" "$R/sys/kernel/iommu_groups/917/devices/0000:f3:00.0"
  mkdir -p "$R/dev/dri/by-path"; : > "$R/dev/dri/card0"; : > "$R/dev/dri/renderD129"
  ln -s ../card0 "$R/dev/dri/by-path/pci-0000:f3:00.0-card"; ln -s ../renderD129 "$R/dev/dri/by-path/pci-0000:f3:00.0-render"
  cat > "$R/sudo" <<EOF
#!/usr/bin/env bash
R=$R
d=\$R/sys/bus/pci/devices/0000:f3:00.0
echo "SUDO \$*" >> \$R/sudo.log
case "\$1" in
  modprobe|chown) exit 0 ;;
  tee) v=\$(cat); printf '%s\n' "\$v" > "\$2"
    case "\$2" in
      */drivers/xe/unbind|*/drivers/vfio-pci/unbind) rm -f "\$d/driver"; rm -f "\$R/dev/vfio/917" ;;
      */drivers_probe) o=\$(cat "\$d/driver_override"); [ -n "\$o" ] || o=xe
        [ -n "\${STUB_NO_REBIND:-}" ] && [ "\$o" = xe ] && exit 0
        ln -s "\$R/sys/bus/pci/drivers/\$o" "\$d/driver"; [ "\$o" = vfio-pci ] && : > "\$R/dev/vfio/917" ;;
    esac ;;
  *) echo "unexpected sudo \$*" >&2; exit 9 ;;
esac
EOF
  chmod +x "$R/sudo"
  # shellcheck disable=SC2016  # $3 expands in the fake binary, not here
  printf '#!/bin/sh\necho "PASS: BAR2 of $3 mapped through VFIO group 917"\n' > "$R/bin-ok"
  # shellcheck disable=SC2016  # $3 expands in the fake binary, not here
  printf '#!/bin/sh\necho "FAIL: open: EACCES"; exit 1\n' > "$R/bin-fail"
  chmod +x "$R/bin-ok" "$R/bin-fail"
  # A fresh session: boot at 1000, the KWin pin written at 1010, kwin_wayland 4242 started at 1000 + 9000/CLK_TCK.
  mkdir -p "$R/proc/4242"; echo "btime 1000" > "$R/proc/stat"; echo kwin_wayland > "$R/proc/4242/comm"
  kwin_stat 9000
  mkdir -p "$R/envd"; echo "KWIN_DRM_DEVICES=/dev/dri/card1" > "$R/envd/60-kwin.conf"; touch -d @1010 "$R/envd/60-kwin.conf"
  echo "EDITOR=vi" > "$R/envd/10-editor.conf"; touch -d @5000 "$R/envd/10-editor.conf"
  printf '#!/bin/sh\necho "4242, /usr/bin/kwin_wayland"\n' > "$R/nvsmi"; chmod +x "$R/nvsmi"
}
kwin_stat() { # start time of the fake kwin_wayland in clock ticks after boot
  local zeros # fields 4 to 21
  zeros=$(printf '0 %.0s' $(seq 4 21))
  echo "4242 (kwin_wayland) S ${zeros}$1 0 0" > "$R/proc/4242/stat"
}
run() {
  local rc=0
  AEGIS_SYSFS=$R/sys AEGIS_DEV=$R/dev AEGIS_SUDO=$R/sudo AEGIS_BIN=$R/${BINNAME:-bin-ok} AEGIS_EVID=$R/evid \
    AEGIS_PROC=$R/proc AEGIS_NVSMI=$R/nvsmi AEGIS_ENVD=$R/envd bash "$S" "$@" > "$R/out" 2>&1 || rc=$?
  echo "$rc"
}
check() { # name expected-rc actual-rc grep-pattern
  if [ "$2" = "$3" ] && grep -q -- "$4" "$R/out"; then echo "ok   $1"; else echo "FAIL $1 (rc $3, want $2; no '$4')"; sed 's/^/     /' "$R/out"; fails=$((fails+1)); fi
}
mk; rc=$(run); check dry-run 0 "$rc" "DRY RUN"; [ ! -e "$R/sudo.log" ] && echo "ok   dry-run-no-sudo" || { echo "FAIL dry-run wrote"; fails=$((fails+1)); }
mk; rc=$(run --apply); check apply-pass 0 "$rc" "RESTORED: 0000:f3:00.0 back on xe"
grep -q "MAP-BAR rc=0" "$R/out" && grep -q "BOUND 0000:f3:00.0 driver=vfio-pci" "$R/out" && grep -q "^RESULT pass" "$R/out" && echo "ok   apply-bound" || { echo "FAIL apply-bound"; fails=$((fails+1)); }
grep -q "nvidia\|f6:00.0" "$R/sudo.log" && { echo "FAIL nvidia written"; fails=$((fails+1)); } || echo "ok   nvidia-never-written"
[ "$(cat "$R/sys/bus/pci/devices/0000:f3:00.0/driver_override")" = "" ] && echo "ok   override-cleared" || { echo "FAIL override"; fails=$((fails+1)); }
ls "$R/evid"/e25-1-vfio-*.log >/dev/null 2>&1 && echo "ok   evidence-log" || { echo "FAIL evidence"; fails=$((fails+1)); }
mk; BINNAME=bin-fail; rc=$(run --apply); BINNAME=; check map-fail-still-restores 1 "$rc" "RESTORED: 0000:f3:00.0 back on xe"
grep -q "RESULT pass" "$R/out" && { echo "FAIL map-fail claimed a pass"; fails=$((fails+1)); } || echo "ok   map-fail-no-result-pass"
mk; rc=$(STUB_NO_REBIND=1 run --apply); check rebind-fail-says-not-restored 3 "$rc" "NOT RESTORED"
mk; echo connected > "$R/sys/bus/pci/devices/0000:f3:00.0/drm/card0/card0-DP-4/status"; rc=$(run --apply); check refuse-connected 2 "$rc" "REFUSE: connected display output: card0-DP-4"
[ ! -e "$R/sudo.log" ] && echo "ok   refuse-connected-no-write" || { echo "FAIL refused but wrote"; fails=$((fails+1)); }
mk; ln -s x "$R/sys/kernel/iommu_groups/917/devices/0000:f3:00.1"; rc=$(run --apply); check refuse-shared-group 2 "$rc" "REFUSE: group 917 holds"
mk; echo 0x10de > "$R/sys/bus/pci/devices/0000:f3:00.0/vendor"; rc=$(run --apply); check refuse-no-arc 2 "$rc" "REFUSE: 0 devices with id 0x8086:0x56a5"
[ ! -e "$R/sudo.log" ] && echo "ok   refuse-no-arc-no-write" || { echo "FAIL refused but wrote"; fails=$((fails+1)); }
mk; mkdir "$R/sys/bus/pci/devices/0000:f4:00.0"; echo 0x8086 > "$R/sys/bus/pci/devices/0000:f4:00.0/vendor"; echo 0x56a5 > "$R/sys/bus/pci/devices/0000:f4:00.0/device"
rc=$(run --apply); check refuse-two-arcs 2 "$rc" "REFUSE: 2 devices with id 0x8086:0x56a5"
mk; rc=$(run); check lists-nvidia-by-vendor 0 "$rc" "0000:f6:00.0 driver=nvidia (NVIDIA, never written)"
mk; rm "$R/sys/bus/pci/devices/0000:f3:00.0/driver"; ln -s "$R/sys/bus/pci/drivers/vfio-pci" "$R/sys/bus/pci/devices/0000:f3:00.0/driver"; rc=$(run --apply); check refuse-not-on-xe 2 "$rc" "not on xe"
mk; rc=$(run --bogus); check usage 64 "$rc" "usage"
# A real process of this test holds a node open, as a desktop compositor may hold the Arc; the guard must name it and refuse.
hold() { # by-path suffix; starts a holder, sets HOLDER
  sleep 60 7< "$R/dev/dri/by-path/pci-0000:f3:00.0-$1" &
  HOLDER=$!
  for _ in $(seq 1 40); do [ -e "/proc/$HOLDER/fd/7" ] && break; sleep 0.05; done
}
unhold() { local hrc=0; kill "$HOLDER"; wait "$HOLDER" 2>/dev/null || hrc=$?; [ "$hrc" != 0 ]; }
mk; hold card; rc=$(run); unhold; check refuse-held-card-dry-run 2 "$rc" "REFUSE: pci-0000:f3:00.0-card held by $HOLDER (sleep)"
mk; hold render; rc=$(run --apply); unhold; check refuse-held-render 2 "$rc" "REFUSE: pci-0000:f3:00.0-render held by $HOLDER (sleep)"
[ ! -e "$R/sudo.log" ] && echo "ok   refuse-held-no-write" || { echo "FAIL held but wrote"; fails=$((fails+1)); }
mk; rm "$R/dev/dri/by-path/pci-0000:f3:00.0-render"; rc=$(run); check refuse-no-by-path 2 "$rc" "REFUSE: no $R/dev/dri/by-path/pci-0000:f3:00.0-render"
# The KWin DRM pin takes effect only at the next login: a kwin_wayland older than the pin may still open the Arc.
mk; kwin_stat 500; rc=$(run --apply); check refuse-stale-kwin 2 "$rc" "REFUSE: kwin_wayland 4242 started 1005, not after the KWin DRM pin"
[ ! -e "$R/sudo.log" ] && echo "ok   refuse-stale-kwin-no-write" || { echo "FAIL stale kwin but wrote"; fails=$((fails+1)); }
mk; kwin_stat 1000; rc=$(run); check refuse-kwin-same-second-as-pin 2 "$rc" "REFUSE: kwin_wayland 4242 started 1010, not after"
mk; kwin_stat 1100; rc=$(run); check kwin-one-second-after-pin 0 "$rc" "kwin_wayland 4242 started 1011, after the KWin DRM pin"
mk; printf '#!/bin/sh\necho "77, /usr/bin/python"\n' > "$R/nvsmi"; rc=$(run --apply); check refuse-kwin-not-on-nvidia 2 "$rc" "REFUSE: nvidia-smi does not list kwin_wayland 4242"
mk; printf '#!/bin/sh\nexit 6\n' > "$R/nvsmi"; rc=$(run); check refuse-nvsmi-fails 2 "$rc" "REFUSE: nvidia-smi failed"
mk; rm -r "$R/proc/4242"; rc=$(run); check refuse-no-kwin 2 "$rc" "REFUSE: no kwin_wayland running"
mk; rm "$R/envd/60-kwin.conf"; rc=$(run --apply); check refuse-no-pin 2 "$rc" "REFUSE: no file in $R/envd sets KWIN_DRM_DEVICES"
mk; sed -i "s/^/# /" "$R/envd/60-kwin.conf"; rc=$(run); check refuse-commented-pin 2 "$rc" "REFUSE: no file in $R/envd sets KWIN_DRM_DEVICES"
mk; echo "KWIN_DRM_DEVICES=/dev/dri/card2" > "$R/envd/70-kwin.conf"; rc=$(run); check refuse-two-pins 2 "$rc" "REFUSE: 2 files in $R/envd set KWIN_DRM_DEVICES"
rm -rf "${STUBS[@]}"
echo "fails=$fails"; [ "$fails" = 0 ]
