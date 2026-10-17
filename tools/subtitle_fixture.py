# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Validate the M32 synchronized subtitle fixture clip and its regression pins.

Milestone M32, slice 2, E32-1. Three sibling streams across a 3.0-second timeline:
- ``subtitle-clip-90.mjpeg``: 90 frames of 256x64 Motion-JPEG video at 30 fps.
- ``subtitle-clip-90.wav``: 48000 Hz mono 16-bit PCM audio (144000 samples).
- ``subtitle-clip-90.srt``: 3-cue SubRip subtitles aligned to whole-frame timecodes.
"""

import hashlib
import re
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES_DIR = ROOT / "crates" / "aegis-scaena" / "fixtures"

VIDEO = FIXTURES_DIR / "subtitle-clip-90.mjpeg"
AUDIO = FIXTURES_DIR / "subtitle-clip-90.wav"
SUBTITLE = FIXTURES_DIR / "subtitle-clip-90.srt"

VIDEO_SHA256 = "499248fe9619c9b9457687cc7abce67e00eb6af23a6285d9532befb3239c493c"
VIDEO_BYTES = 279517

AUDIO_SHA256 = "c45da1e5be8e950fcf01483646b4d8b5e58041986625f86b3cf2a23b366f3597"
AUDIO_BYTES = 288044

SUBTITLE_SHA256 = "20a58bef6628d6628ee523dbcf656d5af8c02aab757cbdc8bcf12143b0917e71"
SUBTITLE_BYTES = 157

FRAME_RATE = 30
FRAMES = 90
MAX_DURATION_MS = (FRAMES * 1000) // FRAME_RATE

AUDIO_SAMPLE_RATE = 48000
AUDIO_CHANNELS = 1
AUDIO_BITS_PER_SAMPLE = 16
AUDIO_SAMPLES = 144000

# Deadlines and bounds (HISS-02).
MAX_SRT_BYTES = 64 * 1024
MAX_SRT_LINES = 1024
MAX_WAV_CHUNKS = 32

VIDEO_COMMAND = (
    "ffmpeg",
    "-nostdin",
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "testsrc=s=256x64:r=30,format=yuvj420p",
    "-frames:v",
    "90",
    "-c:v",
    "mjpeg",
    "-q:v",
    "2",
    "-huffman",
    "default",
    "-fflags",
    "+bitexact",
    "-flags",
    "+bitexact",
    "-f",
    "mjpeg",
    "subtitle-clip-90.mjpeg",
)

AUDIO_COMMAND = (
    "ffmpeg",
    "-nostdin",
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "sine=frequency=1000:sample_rate=48000",
    "-t",
    "3",
    "-c:a",
    "pcm_s16le",
    "-fflags",
    "+bitexact",
    "-flags",
    "+bitexact",
    "-f",
    "wav",
    "subtitle-clip-90.wav",
)

TIMECODE_PATTERN = re.compile(
    r"^(\d{2}):(\d{2}):(\d{2}),(\d{3}) --> (\d{2}):(\d{2}):(\d{2}),(\d{3})$"
)


def check_pin(path, expected_bytes, expected_sha256):
    """Return why a fixture file fails its size or hash pin, or an empty list."""
    target = Path(path)
    if target.is_symlink() or not target.is_file():
        return [f"{target} is missing or a symlink"]
    data = target.read_bytes()
    if len(data) != expected_bytes:
        return [f"{target.name} is {len(data)} bytes, not the pinned {expected_bytes}"]
    digest = hashlib.sha256(data).hexdigest()
    if digest != expected_sha256:
        return [f"{target.name} hashes to {digest}, not the pinned {expected_sha256}"]
    return []


def parse_wav_chunks(data):
    """Scan RIFF WAVE chunks and return (fmt_payload, data_size)."""
    offset = 12
    fmt_payload = None
    data_size = None
    limit = min(len(data), 65536)

    for _ in range(MAX_WAV_CHUNKS):
        if offset + 8 > limit:
            break
        chunk_id = data[offset : offset + 4]
        chunk_sz = struct.unpack("<I", data[offset + 4 : offset + 8])[0]
        payload = data[offset + 8 : offset + 8 + chunk_sz]
        if chunk_id == b"fmt " and len(payload) >= 16:
            fmt_payload = payload[:16]
        elif chunk_id == b"data":
            data_size = chunk_sz
        offset += 8 + chunk_sz
        if offset % 2 == 1:
            offset += 1
        if fmt_payload is not None and data_size is not None:
            break

    return fmt_payload, data_size


def parse_wav_header(data):
    """Parse WAV byte contents into (rate, channels, bits, samples)."""
    if len(data) < 44:
        raise ValueError("WAV data smaller than minimum 44-byte header")
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError("not a RIFF WAVE file")

    fmt_payload, data_size = parse_wav_chunks(data)
    if fmt_payload is None or data_size is None:
        raise ValueError("missing fmt or data chunk in WAV")

    _, channels, rate, _, _, bits = struct.unpack("<HHIIHH", fmt_payload)
    bytes_per_sample = channels * (bits // 8)
    if bytes_per_sample == 0:
        raise ValueError("invalid WAV sample width")
    samples = data_size // bytes_per_sample
    return rate, channels, bits, samples


def check_wav(path=AUDIO):
    """Check that a WAV file header specifies 48000 Hz, mono, 16 bit, 144000 samples."""
    target = Path(path)
    if target.is_symlink() or not target.is_file():
        return [f"{target} is missing or a symlink"]
    data = target.read_bytes()
    try:
        rate, channels, bits, samples = parse_wav_header(data)
    except ValueError as err:
        return [f"{target.name} has malformed WAV header: {err}"]

    problems = []
    if rate != AUDIO_SAMPLE_RATE:
        problems.append(f"{target.name} sample rate is {rate} Hz, not {AUDIO_SAMPLE_RATE} Hz")
    if channels != AUDIO_CHANNELS:
        problems.append(f"{target.name} channel count is {channels}, not {AUDIO_CHANNELS}")
    if bits != AUDIO_BITS_PER_SAMPLE:
        problems.append(f"{target.name} bits per sample is {bits}, not {AUDIO_BITS_PER_SAMPLE}")
    if samples != AUDIO_SAMPLES:
        problems.append(f"{target.name} sample count is {samples}, not {AUDIO_SAMPLES}")
    return problems


def timecode_to_ms(hours, minutes, seconds, millis):
    """Convert timecode digit strings into total milliseconds."""
    h_ms = int(hours) * 3600000
    m_ms = int(minutes) * 60000
    s_ms = int(seconds) * 1000
    return h_ms + m_ms + s_ms + int(millis)


def check_single_timecode(t_ms, text, max_ms):
    """Validate that one timecode is a whole frame at 30 fps and inside 0..max_ms."""
    if (t_ms * FRAME_RATE) % 1000 != 0:
        return f"{text} ({t_ms} ms) is not a whole frame at {FRAME_RATE} fps"
    if t_ms < 0 or t_ms > max_ms:
        return f"{text} ({t_ms} ms) is outside 0..{max_ms} ms"
    return None


def read_srt_lines(path):
    """Read SRT content bounded by MAX_SRT_BYTES and MAX_SRT_LINES."""
    target = Path(path)
    if target.is_symlink() or not target.is_file():
        return None, [f"{target} is missing or a symlink"]
    raw = target.read_bytes()
    if len(raw) > MAX_SRT_BYTES:
        return None, [f"{target.name} is {len(raw)} bytes, exceeding {MAX_SRT_BYTES}"]
    lines = raw.decode("utf-8", errors="replace").splitlines()
    if len(lines) > MAX_SRT_LINES:
        return None, [f"{target.name} has {len(lines)} lines, exceeding {MAX_SRT_LINES}"]
    return lines, []


def check_srt_cues(lines):
    """Parse timecodes in SRT lines and check frame alignment and range."""
    problems = []
    cues_found = 0
    bound = min(len(lines), MAX_SRT_LINES)

    for idx in range(bound):
        line = lines[idx].strip()
        if " --> " in line:
            match = TIMECODE_PATTERN.match(line)
            if not match:
                problems.append(f"malformed timing line: {line}")
                continue
            cues_found += 1
            s_ms = timecode_to_ms(*match.group(1, 2, 3, 4))
            e_ms = timecode_to_ms(*match.group(5, 6, 7, 8))
            s_text = f"{match.group(1)}:{match.group(2)}:{match.group(3)},{match.group(4)}"
            e_text = f"{match.group(5)}:{match.group(6)}:{match.group(7)},{match.group(8)}"

            err = check_single_timecode(s_ms, s_text, MAX_DURATION_MS)
            if err:
                problems.append(err)
            err = check_single_timecode(e_ms, e_text, MAX_DURATION_MS)
            if err:
                problems.append(err)
            if e_ms <= s_ms:
                problems.append(f"cue end {e_text} <= start {s_text}")

    if cues_found == 0 and not problems:
        problems.append("no SRT cues found")
    return problems


def check_srt(path=SUBTITLE):
    """Check that SRT timecodes are whole frames at 30 fps and within timeline bounds."""
    lines, problems = read_srt_lines(path)
    if problems:
        return problems
    return check_srt_cues(lines)


def fixture_problems(path=None, *, video=VIDEO, audio=AUDIO, subtitle=SUBTITLE):
    """Return why the fixture files fail pins or checks, or an empty list."""
    if path is not None:
        target = Path(path)
        if target.name.endswith(".mjpeg"):
            return check_pin(target, VIDEO_BYTES, VIDEO_SHA256)
        if target.name.endswith(".wav"):
            probs = check_pin(target, AUDIO_BYTES, AUDIO_SHA256)
            if target.is_file() and not target.is_symlink():
                probs.extend(check_wav(target))
            return probs
        if target.name.endswith(".srt"):
            probs = check_pin(target, SUBTITLE_BYTES, SUBTITLE_SHA256)
            if target.is_file() and not target.is_symlink():
                probs.extend(check_srt(target))
            return probs
        return [f"{target} has unrecognized fixture extension"]

    problems = []
    problems.extend(check_pin(video, VIDEO_BYTES, VIDEO_SHA256))
    problems.extend(check_pin(audio, AUDIO_BYTES, AUDIO_SHA256))
    problems.extend(check_wav(audio))
    problems.extend(check_pin(subtitle, SUBTITLE_BYTES, SUBTITLE_SHA256))
    problems.extend(check_srt(subtitle))
    return problems
