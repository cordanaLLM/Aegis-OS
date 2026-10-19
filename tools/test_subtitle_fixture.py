# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for the M32 subtitle fixture clip validator and pins (E32-1)."""

import ast
import struct
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

try:
    import subtitle_fixture as fixture
except ImportError:
    from tools import subtitle_fixture as fixture

try:
    from testkit import complexity as count_complexity
except ImportError:
    from tools.testkit import complexity as count_complexity

MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10


class TestSubtitleFixturePositive(unittest.TestCase):
    """Positive tests: committed fixture files and commands match pins and rules."""

    def test_committed_fixtures_pass_all_checks(self):
        """Committed video, audio, and subtitle fixtures pass all checks."""
        self.assertEqual(fixture.fixture_problems(), [])

    def test_committed_pins_individually(self):
        """Each committed file matches its exact byte count and sha256 hash."""
        self.assertEqual(
            fixture.check_pin(fixture.VIDEO, fixture.VIDEO_BYTES, fixture.VIDEO_SHA256),
            [],
        )
        self.assertEqual(
            fixture.check_pin(fixture.AUDIO, fixture.AUDIO_BYTES, fixture.AUDIO_SHA256),
            [],
        )
        self.assertEqual(
            fixture.check_pin(fixture.SUBTITLE, fixture.SUBTITLE_BYTES, fixture.SUBTITLE_SHA256),
            [],
        )

    def test_committed_audio_wav_header(self):
        """Committed audio file header specifies 48000 Hz, mono, 16 bit, 144000 samples."""
        self.assertEqual(fixture.check_wav(fixture.AUDIO), [])

    def test_committed_subtitle_srt_cues(self):
        """Committed subtitle file has 3 cues aligned to whole frames inside 0..3 s."""
        self.assertEqual(fixture.check_srt(fixture.SUBTITLE), [])

    def test_fixture_command_tuples_and_constants(self):
        """Commands are tuples of arguments and frame parameters match specifications."""
        self.assertIsInstance(fixture.VIDEO_COMMAND, tuple)
        self.assertIsInstance(fixture.AUDIO_COMMAND, tuple)
        self.assertGreater(len(fixture.VIDEO_COMMAND), 0)
        self.assertGreater(len(fixture.AUDIO_COMMAND), 0)
        self.assertEqual(fixture.FRAME_RATE, 30)
        self.assertEqual(fixture.FRAMES, 90)


class TestSubtitleFixtureNegative(unittest.TestCase):
    """Negative tests: corrupted pins, invalid timecodes, and bad WAV headers fail."""

    def test_video_pin_fails_on_flipped_byte(self):
        """A video file with one byte flipped fails hash pin."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / fixture.VIDEO.name
            raw = bytearray(fixture.VIDEO.read_bytes())
            raw[1000] ^= 0x01
            copy.write_bytes(bytes(raw))
            problems = fixture.fixture_problems(copy)
            self.assertTrue(problems)
            self.assertIn("not the pinned", problems[0])

    def test_audio_pin_fails_on_flipped_byte(self):
        """An audio file with one byte flipped fails hash pin."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / fixture.AUDIO.name
            raw = bytearray(fixture.AUDIO.read_bytes())
            raw[5000] ^= 0x01
            copy.write_bytes(bytes(raw))
            problems = fixture.fixture_problems(copy)
            self.assertTrue(problems)
            self.assertIn("not the pinned", problems[0])

    def test_subtitle_pin_fails_on_flipped_byte(self):
        """A subtitle file with one byte flipped fails hash pin."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / fixture.SUBTITLE.name
            raw = bytearray(fixture.SUBTITLE.read_bytes())
            raw[50] ^= 0x01
            copy.write_bytes(bytes(raw))
            problems = fixture.fixture_problems(copy)
            self.assertTrue(problems)
            self.assertIn("not the pinned", problems[0])

    def test_srt_with_non_frame_timecode_fails(self):
        """An SRT cue with timecode at 00:00:00,510 fails whole frame alignment."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / "bad.srt"
            bad_content = "1\n00:00:00,510 --> 00:00:01,000\nAegis subtitle non-frame\n\n"
            copy.write_text(bad_content, encoding="utf-8")
            problems = fixture.check_srt(copy)
            self.assertTrue(problems)
            self.assertTrue(any("not a whole frame" in p for p in problems))

    def test_srt_with_dot_timecode_fails(self):
        """An SRT cue with dot separator in timecode fails as malformed timing line."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / "bad.srt"
            bad_content = "1\n00:00:00.500 --> 00:00:01,000\nAegis subtitle dot\n\n"
            copy.write_text(bad_content, encoding="utf-8")
            problems = fixture.check_srt(copy)
            self.assertTrue(problems)
            self.assertTrue(any("malformed timing line" in p for p in problems))
            self.assertFalse(any("no SRT cues found" in p for p in problems))

    def test_wav_header_with_44100_hz_fails(self):
        """A WAV header specifying 44100 Hz fails sample rate check."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / "bad.wav"
            raw = bytearray(fixture.AUDIO.read_bytes())
            struct.pack_into("<I", raw, 24, 44100)
            struct.pack_into("<I", raw, 28, 44100 * 2)
            copy.write_bytes(bytes(raw))
            problems = fixture.check_wav(copy)
            self.assertTrue(problems)
            self.assertTrue(any("44100" in p for p in problems))

    def test_missing_or_symlinked_fixture_fails(self):
        """Absence and symlinks are both refused."""
        with tempfile.TemporaryDirectory() as scratch:
            missing = Path(scratch) / "absent.mjpeg"
            self.assertTrue(fixture.check_pin(missing, 100, "abc"))
            link = Path(scratch) / "link.mjpeg"
            try:
                link.symlink_to(fixture.VIDEO)
                self.assertTrue(fixture.check_pin(link, fixture.VIDEO_BYTES, fixture.VIDEO_SHA256))
            except OSError:
                pass


class TestSubtitleFixtureBoundary(unittest.TestCase):
    """Boundary tests: cues ending at 3.000 s vs 3.100 s, and HISS invariants."""

    def test_cue_ending_exactly_at_three_seconds_passes(self):
        """A cue ending exactly at 3.000 s passes boundary check."""
        with tempfile.TemporaryDirectory() as scratch:
            cue_path = Path(scratch) / "boundary_3000.srt"
            content = "1\n00:00:01,000 --> 00:00:03,000\nBoundary cue\n\n"
            cue_path.write_text(content, encoding="utf-8")
            self.assertEqual(fixture.check_srt(cue_path), [])

    def test_cue_ending_at_three_point_one_seconds_fails(self):
        """A cue ending at 3.100 s exceeds 3.000 s limit and fails."""
        with tempfile.TemporaryDirectory() as scratch:
            cue_path = Path(scratch) / "boundary_3100.srt"
            content = "1\n00:00:01,000 --> 00:00:03,100\nBoundary cue\n\n"
            cue_path.write_text(content, encoding="utf-8")
            problems = fixture.check_srt(cue_path)
            self.assertTrue(problems)
            self.assertTrue(any("outside 0..3000 ms" in p for p in problems))

    def test_no_function_exceeds_length_or_complexity_limit(self):
        """HISS-04: Functions must be <= 60 lines and McCabe complexity <= 10."""
        target_files = [
            TOOLS_DIR / "subtitle_fixture.py",
            TOOLS_DIR / "test_subtitle_fixture.py",
        ]
        offenders = []
        for file_path in target_files:
            tree = ast.parse(file_path.read_text(encoding="utf-8"))
            for node in ast.walk(tree):
                if isinstance(node, ast.FunctionDef):
                    length = node.end_lineno - node.lineno + 1
                    comp = count_complexity(node)
                    if length > MAX_FUNCTION_LINES or comp > MAX_COMPLEXITY:
                        offenders.append(
                            f"{file_path.name}:{node.name}: {length} lines, complexity {comp}"
                        )
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
