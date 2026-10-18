#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for tools/wayland_filter.py."""

import os
import socket
import struct
import threading
import unittest

import wayland_filter
from wayland_filter import Filter, ProtocolError, relay


def pack_string(val: str) -> bytes:
    """Packs a string with uint32 length, NUL byte, and 4-byte padding."""
    raw = val.encode("utf-8") + b"\x00"
    padded_len = (len(raw) + 3) & ~3
    padding = b"\x00" * (padded_len - len(raw))
    return struct.pack("<I", len(raw)) + raw + padding


def pack_msg(obj_id: int, opcode: int, args=None) -> bytes:
    """Packs a Wayland wire message with object id, opcode, and body."""
    body = b"" if args is None else args
    size = 8 + len(body)
    size_opcode = (size << 16) | (opcode & 0xFFFF)
    return struct.pack("<II", obj_id, size_opcode) + body


def make_global(reg_id: int, name: int, interface: str, version: int) -> bytes:
    """Constructs a wl_registry.global server event."""
    body = struct.pack("<I", name) + pack_string(interface) + struct.pack("<I", version)
    return pack_msg(reg_id, 0, body)


def make_bind(reg_id: int, name: int, interface: str, version: int, new_id: int) -> bytes:
    """Constructs a wl_registry.bind client request."""
    body = (
        struct.pack("<I", name)
        + pack_string(interface)
        + struct.pack("<I", version)
        + struct.pack("<I", new_id)
    )
    return pack_msg(reg_id, 0, body)


class TestFilterPositive(unittest.TestCase):
    """Positive tests for Filter modes and event transformation."""

    def setUp(self):
        self.reg_id = 2
        # Client announces get_registry(new_id=2)
        self.init_client_req = pack_msg(1, 1, struct.pack("<I", self.reg_id))
        self.globals = [
            make_global(self.reg_id, 1, "wl_compositor", 4),
            make_global(self.reg_id, 2, "xdg_wm_base", 3),
            make_global(self.reg_id, 3, "zwlr_layer_shell_v1", 4),
            make_global(self.reg_id, 4, "wp_presentation", 1),
        ]
        self.burst = b"".join(self.globals)

    def test_hide_layer_shell_removes_global_only(self):
        filt = Filter("hide-layer-shell")
        filt.from_client(self.init_client_req)

        out = filt.from_server(self.burst)
        expected = self.globals[0] + self.globals[1] + self.globals[3]
        self.assertEqual(out, expected)
        self.assertEqual(filt.dropped, 1)

    def test_drop_mode_keeps_all_globals(self):
        filt = Filter("drop-layer-configure")
        filt.from_client(self.init_client_req)

        out = filt.from_server(self.burst)
        self.assertEqual(out, self.burst)
        self.assertEqual(filt.dropped, 0)

    def test_drop_layer_configure_after_bind(self):
        filt = Filter("drop-layer-configure")
        filt.from_client(self.init_client_req)
        filt.from_server(self.burst)

        # Client binds layer-shell (new id 7) and gets layer surface (new id 9)
        bind_req = make_bind(self.reg_id, 3, "zwlr_layer_shell_v1", 4, 7)
        get_surface_req = pack_msg(7, 0, struct.pack("<I", 9))
        filt.from_client(bind_req + get_surface_req)

        # Server sends configure (opcode 0) on 9, closed (opcode 1) on 9, and configure on 15
        conf_9 = pack_msg(9, 0, struct.pack("<III", 100, 1280, 48))
        closed_9 = pack_msg(9, 1, b"")
        conf_other = pack_msg(15, 0, struct.pack("<I", 101))

        out = filt.from_server(conf_9 + closed_9 + conf_other)
        self.assertEqual(out, closed_9 + conf_other)
        self.assertEqual(filt.dropped, 1)

    def test_hide_mode_never_drops_configure(self):
        filt = Filter("hide-layer-shell")
        filt.from_client(self.init_client_req)
        filt.from_server(self.burst)

        dropped_after_globals = filt.dropped
        bind_req = make_bind(self.reg_id, 3, "zwlr_layer_shell_v1", 4, 7)
        get_surface_req = pack_msg(7, 0, struct.pack("<I", 9))
        filt.from_client(bind_req + get_surface_req)

        conf_9 = pack_msg(9, 0, struct.pack("<III", 100, 1280, 48))
        out = filt.from_server(conf_9)
        self.assertEqual(out, conf_9)
        self.assertEqual(filt.dropped, dropped_after_globals)


class TestFilterNegative(unittest.TestCase):
    """Negative tests for wire framing errors and untracked binds."""

    def test_invalid_size_less_than_8(self):
        filt = Filter("hide-layer-shell")
        bad = struct.pack("<II", 1, (6 << 16) | 0)
        with self.assertRaises(ProtocolError):
            filt.from_server(bad)

    def test_invalid_size_not_multiple_of_4(self):
        filt = Filter("hide-layer-shell")
        bad = struct.pack("<II", 1, (10 << 16) | 0) + b"\x00" * 2
        with self.assertRaises(ProtocolError):
            filt.from_server(bad)

    def test_string_overflow_past_body(self):
        filt = Filter("hide-layer-shell")
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
        # global with string declared length 64 but body length 16
        body = struct.pack("<I", 1) + struct.pack("<I", 64) + b"short\x00\x00\x00"
        bad_msg = pack_msg(2, 0, body)
        with self.assertRaises(ProtocolError):
            filt.from_server(bad_msg)

    def test_bind_unannounced_name_not_tracked(self):
        filt = Filter("drop-layer-configure")
        # Client binds name 99 which was never announced
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
        bind_req = make_bind(2, 99, "other_interface", 1, 7)
        filt.from_client(bind_req)
        # get_layer_surface on 7 should not be tracked as layer surface
        filt.from_client(pack_msg(7, 0, struct.pack("<I", 9)))
        # configure on 9 is not dropped
        conf_9 = pack_msg(9, 0, struct.pack("<III", 100, 1280, 48))
        out = filt.from_server(conf_9)
        self.assertEqual(out, conf_9)
        self.assertEqual(filt.dropped, 0)


class TestObjectIdReuse(unittest.TestCase):
    """wl_display.delete_id frees an id; a later object reusing it is not the old one."""

    def test_reused_registry_id_is_a_new_object(self):
        filt = Filter("drop-layer-configure")
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 39)))
        delete = pack_msg(1, 1, struct.pack("<I", 39))
        done = pack_msg(39, 0, struct.pack("<I", 5))
        self.assertEqual(filt.from_server(delete + done), delete + done)

    def test_reused_layer_surface_id_keeps_its_configure(self):
        filt = Filter("drop-layer-configure")
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
        filt.from_server(make_global(2, 7, "zwlr_layer_shell_v1", 4))
        filt.from_client(make_bind(2, 7, "zwlr_layer_shell_v1", 4, 3))
        filt.from_client(pack_msg(3, 0, struct.pack("<I", 9)))
        filt.from_server(pack_msg(1, 1, struct.pack("<I", 9)))
        event = pack_msg(9, 0, struct.pack("<III", 1, 2, 3))
        self.assertEqual(filt.from_server(event), event)
        self.assertEqual(filt.dropped, 0)

    def test_truncated_delete_id_is_refused(self):
        filt = Filter("hide-layer-shell")
        with self.assertRaises(ProtocolError):
            filt.from_server(pack_msg(1, 1))


class TestFilterBoundary(unittest.TestCase):
    """Boundary tests for byte splitting, minimal headers, and limits."""

    def test_message_split_at_every_offset(self):
        filt_ref = Filter("hide-layer-shell")
        filt_ref.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
        burst = make_global(2, 1, "wl_compositor", 4) + make_global(2, 2, "zwlr_layer_shell_v1", 4)
        reference_out = filt_ref.from_server(burst)

        for split_pos in range(len(burst) + 1):
            filt = Filter("hide-layer-shell")
            filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
            chunk1 = filt.from_server(burst[:split_pos])
            chunk2 = filt.from_server(burst[split_pos:])
            self.assertEqual(chunk1 + chunk2, reference_out, f"failed at offset {split_pos}")

    def test_exactly_8_byte_message_passes(self):
        filt = Filter("hide-layer-shell")
        minimal = pack_msg(1, 0, b"")
        out = filt.from_client(minimal)
        self.assertEqual(out, minimal)

    def test_max_tracked_exceeded(self):
        filt = Filter("hide-layer-shell")
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))
        # Announce globals up to MAX_TRACKED + 1
        with self.assertRaises(ProtocolError):
            for i in range(wayland_filter.MAX_TRACKED + 2):
                msg = make_global(2, i, f"intf_{i}", 1)
                filt.from_server(msg)


@unittest.skipUnless(
    hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS"),
    "AF_UNIX and SCM_RIGHTS are POSIX-only",
)
class TestRelaySocket(unittest.TestCase):
    """Socket relay tests with fd passing over AF_UNIX."""

    def test_relay_forwards_bytes_and_fds(self):
        c_sock, c_peer = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        s_sock, s_peer = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)

        filt = Filter("hide-layer-shell")
        filt.from_client(pack_msg(1, 1, struct.pack("<I", 2)))

        t = threading.Thread(
            target=relay,
            args=(c_peer, s_peer, filt, 5.0),
            daemon=True,
        )
        t.start()

        pipe_r, pipe_w = os.pipe()
        os.write(pipe_w, b"SCM_RIGHTS_TEST_DATA")
        os.close(pipe_w)

        try:
            # Server sends globals and attaches pipe_r fd
            burst = make_global(2, 1, "wl_compositor", 4) + make_global(
                2, 2, "zwlr_layer_shell_v1", 4
            )
            ancillary = [
                (
                    socket.SOL_SOCKET,
                    socket.SCM_RIGHTS,
                    struct.pack("<i", pipe_r),
                )
            ]
            s_sock.sendmsg([burst], ancillary)

            # Client receives transformed bytes and the fd
            cmsg_space = socket.CMSG_SPACE(struct.calcsize("i"))
            data, ancdata, _, _ = c_sock.recvmsg(4096, cmsg_space)

            # zwlr_layer_shell_v1 is hidden; only wl_compositor arrives
            expected_bytes = make_global(2, 1, "wl_compositor", 4)
            self.assertEqual(data, expected_bytes)

            received_fds = []
            for level, typ, cdata in ancdata:
                if level == socket.SOL_SOCKET and typ == socket.SCM_RIGHTS:
                    received_fds.append(struct.unpack("<i", cdata)[0])

            self.assertEqual(len(received_fds), 1)
            fd = received_fds[0]
            pipe_data = os.read(fd, 32)
            self.assertEqual(pipe_data, b"SCM_RIGHTS_TEST_DATA")
            os.close(fd)
        finally:
            os.close(pipe_r)
            c_sock.close()
            s_sock.close()
            t.join(timeout=2.0)


@unittest.skipUnless(
    hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS"),
    "AF_UNIX and SCM_RIGHTS are POSIX-only",
)
class TestRelayReads(unittest.TestCase):
    """A read that would block keeps the relay open; only a closed peer ends it."""

    def test_would_block_is_not_end_of_stream(self):
        a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            a.setblocking(False)
            self.assertIsNone(wayland_filter._recv_with_fds(a))
            filt = Filter("hide-layer-shell")
            self.assertTrue(wayland_filter._handle_client_read(a, b, filt, bytearray(), []))
        finally:
            a.close()
            b.close()

    def test_closed_peer_ends_the_relay(self):
        a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        b.close()
        try:
            filt = Filter("hide-layer-shell")
            self.assertFalse(wayland_filter._handle_client_read(a, a, filt, bytearray(), []))
        finally:
            a.close()

    def test_pending_bytes_flush_without_a_new_read(self):
        a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            pending = bytearray(b"\x01\x00\x00\x00\x00\x00\x08\x00")
            wayland_filter._flush_pending([(a, pending, [])])
            self.assertEqual(pending, bytearray())
            self.assertEqual(b.recv(16), b"\x01\x00\x00\x00\x00\x00\x08\x00")
        finally:
            a.close()
            b.close()


if __name__ == "__main__":
    unittest.main()
