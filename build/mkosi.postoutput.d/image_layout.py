#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Refuse a disk image whose ESP holds no UKI or whose root-a is near-empty (M11).

Imago run 39090040772 built aegis-os.raw from build/repart.d at 2e7b59a and
booted it: the firmware answered "No bootable option or device was found",
because 00-esp.conf copied only /boot while mkosi 27.1 writes the UKI to
/efi/EFI/BOOT/BOOTX64.EFI, and 10-root-a.conf copied nothing, so root-a was a
4K EROFS. The build itself exited 0. mkosi runs every file under
mkosi.postoutput.d/ after it writes the image (PostOutputScripts=, mkosi(1)
27.1), with the staged output under $OUTPUTDIR, so this check fails the build
instead of a boot.

It reads metadata only, with the standard library, because it runs inside the
mkosi tools tree and its tests run on every host: the GPT, the FAT32 directory
path EFI/BOOT/BOOTX64.EFI with the PE "MZ" header of its first cluster, and the
EROFS superblock of the partition labelled root-a. Sizes and layouts follow the
UEFI specification 2.10 (GPT, section 5.3; the ESP is FAT32 on fixed media,
section 13.3.1.1), the Microsoft FAT specification of 2005, and
include/erofs_fs.h of erofs-utils 1.9.

The root floors are well under any root holding systemd and linux-rt (a module
tree alone has thousands of files) and far above the one-inode, one-block
volume the failed run produced. A gate that stages a small tree passes lower
floors with --min-root-inodes= and --min-root-bytes=.
"""

import os
import struct
import sys
import uuid
from pathlib import Path

SECTOR = 512
ESP_TYPE = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
ROOT_TYPE = uuid.UUID("4f68bce3-e8cd-4db1-96e7-fbcaf984b709")
ROOT_LABEL = "root-a"
UKI_PATH = (b"EFI        ", b"BOOT       ", b"BOOTX64 EFI")
UKI_NAME = "EFI/BOOT/BOOTX64.EFI"
EROFS_MAGIC = 0xE0F5E1E2
EROFS_SUPERBLOCK = 1024
MIN_ROOT_INODES = 1000
MIN_ROOT_BYTES = 64 * 1024 * 1024
MAX_ENTRIES = 256
MAX_DIR_CLUSTERS = 4096
FAT32_END = 0x0FFFFFF8
ATTR_DIR = 0x10
ATTR_LFN = 0x0F
ATTR_VOLUME = 0x08
USAGE = "usage: image_layout.py [--min-root-inodes=N] [--min-root-bytes=N] [IMAGE...]"


def read_at(handle, offset, size):
    """Return `size` bytes at `offset`, short only at the end of the file."""
    handle.seek(offset)
    return handle.read(size)


def partitions(handle):
    """Return (type uuid, label, byte offset) per used GPT entry, or None without a GPT."""
    header = read_at(handle, SECTOR, 92)
    if len(header) < 92 or header[:8] != b"EFI PART":
        return None
    entries_lba, count, entry_size = struct.unpack_from("<QII", header, 72)
    if not 128 <= entry_size <= 1024:
        return None
    table = read_at(handle, entries_lba * SECTOR, min(count, MAX_ENTRIES) * entry_size)
    found = []
    for start in range(0, len(table) - entry_size + 1, entry_size):
        type_uuid = uuid.UUID(bytes_le=table[start : start + 16])
        if type_uuid.int == 0:
            continue
        first_lba = struct.unpack_from("<Q", table, start + 32)[0]
        label = table[start + 56 : start + 128].decode("utf-16-le", "replace").rstrip("\0")
        found.append((type_uuid, label, first_lba * SECTOR))
    return found


class Fat32:
    """The FAT32 volume at `offset`: cluster chains and directory lookups."""

    def __init__(self, handle, offset):
        boot = read_at(handle, offset, 64)
        if len(boot) < 64:
            raise ValueError("not a FAT volume (short boot sector)")
        sector, per_cluster, reserved, fats = struct.unpack_from("<HBHB", boot, 11)
        fat16_size = struct.unpack_from("<H", boot, 22)[0]
        fat_size, self.root = struct.unpack_from("<I4xI", boot, 36)
        if sector not in (512, 1024, 2048, 4096) or per_cluster == 0 or fats == 0:
            raise ValueError("not a FAT volume (no BIOS parameter block)")
        if fat16_size != 0 or fat_size == 0:
            raise ValueError("not FAT32, which UEFI requires for an ESP on fixed media")
        self.handle = handle
        self.fat = offset + reserved * sector
        self.cluster_bytes = per_cluster * sector
        self.data = self.fat + fats * fat_size * sector

    def cluster_offset(self, cluster):
        return self.data + (cluster - 2) * self.cluster_bytes

    def chain(self, cluster):
        """Yield the clusters of the chain starting at `cluster`, at most MAX_DIR_CLUSTERS."""
        for _ in range(MAX_DIR_CLUSTERS):
            if cluster < 2 or cluster >= FAT32_END:
                return
            yield cluster
            entry = read_at(self.handle, self.fat + 4 * cluster, 4)
            if len(entry) < 4:
                return
            cluster = struct.unpack("<I", entry)[0] & 0x0FFFFFFF

    def lookup(self, directory, name):
        """Return (attr, first cluster, size) of short name `name` in `directory`, or None."""
        for cluster in self.chain(directory):
            block = read_at(self.handle, self.cluster_offset(cluster), self.cluster_bytes)
            for start in range(0, len(block) - 31, 32):
                if block[start] == 0:
                    return None
                attr = block[start + 11]
                if block[start] == 0xE5 or attr == ATTR_LFN or attr & ATTR_VOLUME:
                    continue
                if block[start : start + 11].upper() == name:
                    high, low, size = struct.unpack_from("<H4xHI", block, start + 20)
                    return attr, (high << 16) | low, size
        return None


def uki_problems(handle, offset):
    """Return the ESP's problems: FAT32 holding a PE image at EFI/BOOT/BOOTX64.EFI."""
    try:
        volume = Fat32(handle, offset)
    except ValueError as error:
        return [f"ESP: {error}"]
    directory, entry = volume.root, None
    for depth, name in enumerate(UKI_PATH):
        entry = volume.lookup(directory, name)
        last = depth == len(UKI_PATH) - 1
        if entry is None or bool(entry[0] & ATTR_DIR) == last:
            return [f"ESP: {UKI_NAME} is missing"]
        directory = entry[1]
    if entry is None:
        return [f"ESP: {UKI_NAME} is missing"]
    _, cluster, size = entry
    if size == 0:
        return [f"ESP: {UKI_NAME} is empty"]
    if read_at(handle, volume.cluster_offset(cluster), 2) != b"MZ":
        return [f"ESP: {UKI_NAME} is not a PE image (no MZ header)"]
    return []


def root_problems(handle, offset, min_inodes, min_bytes):
    """Return root-a's problems: an EROFS superblock with at least the floor's content."""
    block = read_at(handle, offset + EROFS_SUPERBLOCK, 40)
    if len(block) < 40 or struct.unpack_from("<I", block)[0] != EROFS_MAGIC:
        return [f"{ROOT_LABEL}: no EROFS superblock"]
    size_bits = block[12]
    inodes = struct.unpack_from("<Q", block, 16)[0]
    blocks = struct.unpack_from("<I", block, 36)[0]
    size = blocks << size_bits if 9 <= size_bits <= 16 else 0
    if inodes < min_inodes or size < min_bytes:
        return [
            f"{ROOT_LABEL}: near-empty EROFS ({inodes} inodes, {size} bytes; "
            f"floor {min_inodes} inodes, {min_bytes} bytes)"
        ]
    return []


def problems(path, min_inodes=MIN_ROOT_INODES, min_bytes=MIN_ROOT_BYTES):
    """Return every layout problem of the disk image at `path`; empty means bootable layout."""
    with open(path, "rb") as handle:
        table = partitions(handle)
        if table is None:
            return ["no GPT header at LBA 1"]
        esp = [offset for kind, _, offset in table if kind == ESP_TYPE]
        root = [
            offset for kind, label, offset in table if kind == ROOT_TYPE and label == ROOT_LABEL
        ]
        found = uki_problems(handle, esp[0]) if esp else ["no EFI system partition"]
        if not root:
            return found + [f"no root partition labelled {ROOT_LABEL}"]
        return found + root_problems(handle, root[0], min_inodes, min_bytes)


def parse(argv):
    """Return (images, min inodes, min bytes) from `argv`; ValueError on a bad argument."""
    images, floors = [], {"--min-root-inodes": MIN_ROOT_INODES, "--min-root-bytes": MIN_ROOT_BYTES}
    for argument in argv[:MAX_ENTRIES]:
        key, _, value = argument.partition("=")
        if key in floors:
            floors[key] = int(value)
        elif argument.startswith("-"):
            raise ValueError(argument)
        else:
            images.append(Path(argument))
    return images, floors["--min-root-inodes"], floors["--min-root-bytes"]


def staged_images(env):
    """Return the one .raw image mkosi staged under $OUTPUTDIR; ValueError otherwise."""
    if not env.get("OUTPUTDIR"):
        raise ValueError("OUTPUTDIR is not set; mkosi sets it for post-output scripts")
    found = sorted(Path(env["OUTPUTDIR"]).glob("*.raw"))
    if len(found) != 1:
        raise ValueError(
            f"expected exactly one .raw image in {env['OUTPUTDIR']}, found {len(found)}"
        )
    return found


def main(argv, env, out):
    """Check the named images, or the image mkosi staged; 0 ok, 1 refused, 2 usage."""
    try:
        images, min_inodes, min_bytes = parse(argv)
    except ValueError:
        print(USAGE, file=out)
        return 2
    try:
        images = images or staged_images(env)
    except ValueError as error:
        print(f"FAIL: {error}", file=out)
        return 1
    failed = False
    for path in images:
        try:
            found = problems(path, min_inodes, min_bytes)
        except OSError as error:
            found = [f"could not be read: {error}"]
        for problem in found:
            print(f"FAIL: {path.name}: {problem}", file=out)
        if not found:
            print(f"ok: {path.name}: ESP holds {UKI_NAME}, {ROOT_LABEL} holds a root", file=out)
        failed = failed or bool(found)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:], os.environ, sys.stdout))
