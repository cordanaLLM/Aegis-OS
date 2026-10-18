# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary cases for the image layout check (M11).

Imago run 39090040772 booted an aegis-os.raw whose ESP held no UKI and whose
root-a was a 4K EROFS, and the firmware answered "No bootable option". The
check in build/mkosi.postoutput.d/image_layout.py refuses such an image before
it reaches a boot. The synthetic images below carry only the metadata the
check reads, so they run on every host; the systemd-repart cases build real
images from build/repart.d and skip with a printed reason where its tools are
missing.
"""

import importlib.util
import io
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECK = ROOT / "build" / "mkosi.postoutput.d" / "image_layout.py"
REPART = ROOT / "build" / "repart.d"
SECTOR = 512
ESP_TYPE = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
ROOT_TYPE = uuid.UUID("4f68bce3-e8cd-4db1-96e7-fbcaf984b709")
VERITY_TYPE = uuid.UUID("2c7357ed-ebd2-46d9-aec1-23d437ec2bf5")
ESP_LBA = 2048
ROOT_LBA = 8192
EROFS_MAGIC = 0xE0F5E1E2
SEED = "1d8b8b28-0b1f-4c1a-9a5f-2a0c3e4d5f60"
REPART_TOOLS = ("systemd-repart", "mkfs.vfat", "mkfs.erofs", "mcopy")


def load_check():
    """Load the check without writing bytecode into the mkosi script directory.

    mkosi runs every file under mkosi.postoutput.d/, so a __pycache__ left there
    by an import would be handed to it as a script.
    """
    spec = importlib.util.spec_from_file_location("image_layout", CHECK)
    module = importlib.util.module_from_spec(spec)
    previous, sys.dont_write_bytecode = sys.dont_write_bytecode, True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


layout = load_check()


def gpt(entries):
    """Return a protective MBR, GPT header and entry array for `entries`."""
    table = bytearray(128 * 128)
    for index, (type_uuid, label, first, last) in enumerate(entries):
        name = label.encode("utf-16-le").ljust(72, b"\0")
        struct.pack_into(
            "<16s16sQQQ72s",
            table,
            index * 128,
            type_uuid.bytes_le,
            uuid.uuid4().bytes_le,
            first,
            last,
            0,
            name,
        )
    header = bytearray(SECTOR)
    struct.pack_into(
        "<8sIIIIQQQQ16sQIII",
        header,
        0,
        b"EFI PART",
        0x10000,
        92,
        0,
        0,
        1,
        0,
        34,
        0,
        uuid.uuid4().bytes_le,
        2,
        128,
        128,
        0,
    )
    return bytes(SECTOR) + bytes(header) + bytes(table)


def dirent(name, attr, cluster, size):
    """Return one 32-byte FAT short-name directory entry."""
    return struct.pack(
        "<11sB8sH4sHI", name, attr, bytes(8), cluster >> 16, bytes(4), cluster & 0xFFFF, size
    )


def fat32(files):
    """Return a FAT32 volume holding /EFI/BOOT/<name> for each (name, data) in `files`.

    One sector per cluster, 32 reserved sectors, one FAT of 8 sectors, so cluster
    N starts at sector 40 + (N - 2). Clusters 2..4 hold the root, EFI and BOOT
    directories; file data starts at cluster 5, one cluster per file.
    """
    volume = bytearray(SECTOR * 64)
    struct.pack_into(
        "<3s8sHBHBHHBHHHII",
        volume,
        0,
        b"\xeb\x58\x90",
        b"mkfs.fat",
        SECTOR,
        1,
        32,
        1,
        0,
        0,
        0xF8,
        0,
        0,
        0,
        0,
        64,
    )
    struct.pack_into("<IHHI", volume, 36, 8, 0, 0, 2)
    fat = [0x0FFFFFF8, 0x0FFFFFFF] + [0x0FFFFFFF] * (3 + len(files))
    struct.pack_into(f"<{len(fat)}I", volume, 32 * SECTOR, *fat)

    def cluster(number):
        return (40 + number - 2) * SECTOR

    volume[cluster(2) : cluster(2) + 32] = dirent(b"EFI        ", 0x10, 3, 0)
    volume[cluster(3) : cluster(3) + 32] = dirent(b"BOOT       ", 0x10, 4, 0)
    for index, (name, data) in enumerate(files):
        volume[cluster(4) + 32 * index : cluster(4) + 32 * index + 32] = dirent(
            name, 0x20, 5 + index, len(data)
        )
        volume[cluster(5 + index) : cluster(5 + index) + len(data)] = data
    return bytes(volume)


def erofs(inodes, blocks, magic=EROFS_MAGIC):
    """Return the first 2 KiB of an EROFS volume: its superblock at offset 1024."""
    head = bytearray(2048)
    struct.pack_into("<IIIBBHQQII", head, 1024, magic, 0, 0, 12, 0, 36, inodes, 0, 0, blocks)
    return bytes(head)


def image(directory, esp=None, root=None, entries=None):
    """Write a sparse image with `esp` at ESP_LBA and `root` at ROOT_LBA."""
    if entries is None:
        entries = [(ESP_TYPE, "ESP", ESP_LBA, ROOT_LBA - 1), (ROOT_TYPE, "root-a", ROOT_LBA, 16383)]
    path = Path(directory) / "aegis-os.raw"
    with path.open("wb") as handle:
        handle.write(gpt(entries))
        for lba, payload in ((ESP_LBA, esp), (ROOT_LBA, root)):
            if payload is not None:
                handle.seek(lba * SECTOR)
                handle.write(payload)
        handle.truncate(16384 * SECTOR)
    return path


UKI = b"MZ" + bytes(SECTOR - 2)
GOOD_ESP = fat32([(b"BOOTX64 EFI", UKI)])
GOOD_ROOT = erofs(layout.MIN_ROOT_INODES, layout.MIN_ROOT_BYTES // 4096)


class LayoutCases(unittest.TestCase):
    def check(self, **parts):
        with tempfile.TemporaryDirectory() as directory:
            return layout.problems(image(directory, **parts))

    def test_positive_full_layout(self):
        self.assertEqual(self.check(esp=GOOD_ESP, root=GOOD_ROOT), [])

    def test_negative_esp_without_uki(self):
        problems = self.check(esp=fat32([]), root=GOOD_ROOT)
        self.assertEqual(problems, ["ESP: EFI/BOOT/BOOTX64.EFI is missing"])

    def test_negative_esp_unformatted(self):
        problems = self.check(esp=None, root=GOOD_ROOT)
        self.assertEqual(len(problems), 1)
        self.assertIn("ESP: not a FAT volume", problems[0])

    def test_negative_uki_not_pe(self):
        problems = self.check(esp=fat32([(b"BOOTX64 EFI", b"\0" * 64)]), root=GOOD_ROOT)
        self.assertEqual(problems, ["ESP: EFI/BOOT/BOOTX64.EFI is not a PE image (no MZ header)"])

    def test_negative_uki_empty(self):
        problems = self.check(esp=fat32([(b"BOOTX64 EFI", b"")]), root=GOOD_ROOT)
        self.assertEqual(problems, ["ESP: EFI/BOOT/BOOTX64.EFI is empty"])

    def test_negative_root_4k_like_run_39090040772(self):
        problems = self.check(esp=GOOD_ESP, root=erofs(1, 1))
        self.assertEqual(len(problems), 1)
        self.assertIn("root-a: near-empty", problems[0])

    def test_negative_root_not_erofs(self):
        problems = self.check(esp=GOOD_ESP, root=erofs(5000, 50000, magic=0))
        self.assertEqual(problems, ["root-a: no EROFS superblock"])

    def test_negative_both_like_run_39090040772(self):
        problems = self.check(esp=fat32([]), root=erofs(1, 1))
        self.assertEqual(len(problems), 2)

    def test_negative_no_root_a(self):
        entries = [(ESP_TYPE, "ESP", ESP_LBA, ROOT_LBA - 1), (ROOT_TYPE, "root-b", ROOT_LBA, 16383)]
        problems = self.check(esp=GOOD_ESP, root=GOOD_ROOT, entries=entries)
        self.assertEqual(problems, ["no root partition labelled root-a"])

    def test_negative_no_esp(self):
        entries = [
            (VERITY_TYPE, "ESP", ESP_LBA, ROOT_LBA - 1),
            (ROOT_TYPE, "root-a", ROOT_LBA, 16383),
        ]
        problems = self.check(esp=GOOD_ESP, root=GOOD_ROOT, entries=entries)
        self.assertEqual(problems, ["no EFI system partition"])

    def test_negative_no_gpt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "aegis-os.raw"
            path.write_bytes(bytes(SECTOR * 4))
            self.assertEqual(layout.problems(path), ["no GPT header at LBA 1"])

    def test_boundary_root_at_floor_passes_one_below_fails(self):
        floor_blocks = layout.MIN_ROOT_BYTES // 4096
        self.assertEqual(
            self.check(esp=GOOD_ESP, root=erofs(layout.MIN_ROOT_INODES, floor_blocks)), []
        )
        below_inodes = self.check(
            esp=GOOD_ESP, root=erofs(layout.MIN_ROOT_INODES - 1, floor_blocks)
        )
        below_bytes = self.check(esp=GOOD_ESP, root=erofs(layout.MIN_ROOT_INODES, floor_blocks - 1))
        self.assertEqual(len(below_inodes), 1)
        self.assertEqual(len(below_bytes), 1)

    def test_boundary_uki_found_among_other_entries(self):
        esp = fat32([(b"README  TXT", b"x"), (b"BOOTX64 EFI", UKI)])
        self.assertEqual(self.check(esp=esp, root=GOOD_ROOT), [])


class MainCases(unittest.TestCase):
    def run_main(self, argv, env):
        out = io.StringIO()
        code = layout.main(argv, env, out)
        return code, out.getvalue()

    def test_positive_outputdir_single_raw(self):
        with tempfile.TemporaryDirectory() as directory:
            image(directory, esp=GOOD_ESP, root=GOOD_ROOT)
            code, text = self.run_main([], {"OUTPUTDIR": directory})
        self.assertEqual(code, 0, text)
        self.assertIn("ok: aegis-os.raw", text)

    def test_negative_outputdir_planted_empty_esp(self):
        with tempfile.TemporaryDirectory() as directory:
            image(directory, esp=fat32([]), root=GOOD_ROOT)
            code, text = self.run_main([], {"OUTPUTDIR": directory})
        self.assertEqual(code, 1)
        self.assertIn("FAIL: aegis-os.raw: ESP: EFI/BOOT/BOOTX64.EFI is missing", text)

    def test_negative_outputdir_without_raw(self):
        with tempfile.TemporaryDirectory() as directory:
            code, text = self.run_main([], {"OUTPUTDIR": directory})
        self.assertEqual(code, 1)
        self.assertIn("exactly one .raw", text)

    def test_negative_outputdir_two_raw(self):
        with tempfile.TemporaryDirectory() as directory:
            image(directory, esp=GOOD_ESP, root=GOOD_ROOT)
            (Path(directory) / "second.raw").write_bytes(b"")
            code, text = self.run_main([], {"OUTPUTDIR": directory})
        self.assertEqual(code, 1)
        self.assertIn("found 2", text)

    def test_negative_no_outputdir(self):
        code, text = self.run_main([], {})
        self.assertEqual(code, 1)
        self.assertIn("OUTPUTDIR", text)

    def test_boundary_explicit_floors(self):
        with tempfile.TemporaryDirectory() as directory:
            path = image(directory, esp=GOOD_ESP, root=erofs(8, 2))
            code, text = self.run_main(
                ["--min-root-inodes=8", "--min-root-bytes=8192", str(path)], {}
            )
            self.assertEqual(code, 0, text)
            code, _ = self.run_main(["--min-root-inodes=9", "--min-root-bytes=8192", str(path)], {})
            self.assertEqual(code, 1)

    def test_negative_bad_argument(self):
        code, text = self.run_main(["--min-root-inodes=x", "a.raw"], {})
        self.assertEqual(code, 2)
        self.assertIn("usage", text)


def missing_repart_tools():
    return [tool for tool in REPART_TOOLS if shutil.which(tool) is None]


class RepartCases(unittest.TestCase):
    """Real images from build/repart.d: the fixed layout green, the run's layout red."""

    def setUp(self):
        missing = missing_repart_tools()
        if missing:
            print(f"SKIP: repart image cases need {', '.join(missing)} on PATH")
            self.skipTest(f"missing {missing}")
        self.base = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.base, True)
        root = self.base / "root"
        for sub in ("boot", "etc", "efi/EFI/BOOT", "usr/bin"):
            (root / sub).mkdir(parents=True)
        (root / "etc" / "machine-id").write_text("ffffffffffffffffffffffffffffffff\n")
        (root / "efi" / "EFI" / "BOOT" / "BOOTX64.EFI").write_bytes(UKI * 64)
        (root / "usr" / "bin" / "payload").write_bytes(bytes(range(256)) * 512)
        self.root = root

    def build(self, definitions):
        output = self.base / "aegis-os.raw"
        argv = [
            "systemd-repart",
            "--empty=create",
            "--size=2G",
            "--offline=yes",
            f"--seed={SEED}",
            f"--root={self.root}",
            f"--definitions={definitions}",
            str(output),
        ]
        result = subprocess.run(argv, capture_output=True, text=True, timeout=300, check=False)
        self.assertEqual(result.returncode, 0, result.stderr[-2000:])
        return output

    def floors(self, path):
        return layout.problems(path, min_inodes=4, min_bytes=4096)

    def test_positive_committed_definitions(self):
        self.assertEqual(self.floors(self.build(REPART)), [])

    def test_negative_planted_layout_of_run_39090040772(self):
        planted = self.base / "planted"
        shutil.copytree(REPART, planted)
        for name, old in (
            ("00-esp.conf", "CopyFiles=/efi:/\n"),
            ("10-root-a.conf", "CopyFiles=/\n"),
        ):
            text = (planted / name).read_text(encoding="utf-8")
            self.assertIn(old, text)
            (planted / name).write_text(text.replace(old, ""), encoding="utf-8")
        problems = self.floors(self.build(planted))
        self.assertIn("ESP: EFI/BOOT/BOOTX64.EFI is missing", problems)
        self.assertTrue(any(p.startswith("root-a: near-empty") for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
