#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Wayland protocol filter proxy for testing client surface and error behaviors.

Intercepts Wayland wire traffic on an AF_UNIX stream socket, tracking registry
globals and layer-shell objects. Supports hiding `zwlr_layer_shell_v1` from
advertised globals or dropping `zwlr_layer_surface_v1.configure` events.
"""

import argparse
import os
import select
import selectors
import socket
import struct
import sys
import threading
import time

MODES = ("hide-layer-shell", "drop-layer-configure")
LAYER_SHELL = "zwlr_layer_shell_v1"
MAX_TRACKED = 4096
MAX_BUFFER = 65536 + 8
MAX_MSGS_PER_CHUNK = 8192
MAX_RELAY_ITERATIONS = 500_000


class ProtocolError(Exception):
    """Raised when wire data violates Wayland protocol framing."""


class Filter:
    """Pure byte transformer tracking Wayland registry and layer-shell objects."""

    def __init__(self, mode):
        if mode not in MODES:
            raise ValueError(f"unknown mode: {mode}; expected one of {MODES}")
        self.mode = mode
        self.dropped = 0
        self._registry_ids = set()
        self._layer_shell_ids = set()
        self._layer_surface_ids = set()
        self._globals = {}  # uint name -> str interface
        self._client_buf = bytearray()
        self._server_buf = bytearray()

    def _check_capacity(self):
        total = (
            len(self._registry_ids)
            + len(self._layer_shell_ids)
            + len(self._layer_surface_ids)
            + len(self._globals)
        )
        if total > MAX_TRACKED:
            raise ProtocolError(f"tracked objects cap exceeded: {total} > {MAX_TRACKED}")

    @staticmethod
    def _parse_string(body, off):
        if off + 4 > len(body):
            raise ProtocolError("string length field truncated")
        str_len = struct.unpack_from("<I", body, off)[0]
        if str_len == 0:
            return "", off + 4
        padded = (str_len + 3) & ~3
        if off + 4 + padded > len(body):
            raise ProtocolError("string content overflows message body")
        raw = body[off + 4 : off + 4 + str_len]
        if not raw.endswith(b"\x00"):
            raise ProtocolError("unterminated string argument")
        val = raw[:-1].decode("utf-8", errors="replace")
        return val, off + 4 + padded

    def _process_client_msg(self, obj_id, opcode, body):
        if obj_id == 1 and opcode == 1:
            if len(body) < 4:
                raise ProtocolError("wl_display.get_registry truncated")
            reg_id = struct.unpack_from("<I", body, 0)[0]
            self._registry_ids.add(reg_id)
            self._check_capacity()
        elif obj_id in self._registry_ids and opcode == 0:
            if len(body) < 4:
                raise ProtocolError("wl_registry.bind truncated")
            name = struct.unpack_from("<I", body, 0)[0]
            interface, off = self._parse_string(body, 4)
            if off + 8 > len(body):
                raise ProtocolError("wl_registry.bind truncated after interface")
            new_id = struct.unpack_from("<I", body, off + 4)[0]
            if self._globals.get(name) == LAYER_SHELL or interface == LAYER_SHELL:
                self._layer_shell_ids.add(new_id)
                self._check_capacity()
        elif obj_id in self._layer_shell_ids and opcode == 0:
            if len(body) < 4:
                raise ProtocolError("zwlr_layer_shell_v1.get_layer_surface truncated")
            surface_id = struct.unpack_from("<I", body, 0)[0]
            self._layer_surface_ids.add(surface_id)
            self._check_capacity()

    def from_client(self, data: bytes) -> bytes:
        """Buffers client bytes and passes through complete messages, updating state."""
        self._client_buf.extend(data)
        if len(self._client_buf) > MAX_BUFFER:
            raise ProtocolError("client buffer overflow")
        out = bytearray()
        for _ in range(MAX_MSGS_PER_CHUNK):
            if len(self._client_buf) < 8:
                break
            size = struct.unpack_from("<I", self._client_buf, 4)[0] >> 16
            if size < 8 or size % 4 != 0:
                raise ProtocolError(f"invalid client message size: {size}")
            if len(self._client_buf) < size:
                break
            msg = bytes(self._client_buf[:size])
            del self._client_buf[:size]
            obj_id = struct.unpack_from("<I", msg, 0)[0]
            opcode = struct.unpack_from("<I", msg, 4)[0] & 0xFFFF
            self._process_client_msg(obj_id, opcode, msg[8:])
            out.extend(msg)
        return bytes(out)

    def _forget(self, body):
        """wl_display.delete_id: the id is free, and a later object reusing it is a new one."""
        if len(body) < 4:
            raise ProtocolError("wl_display.delete_id truncated")
        object_id = struct.unpack_from("<I", body, 0)[0]
        self._registry_ids.discard(object_id)
        self._layer_shell_ids.discard(object_id)
        self._layer_surface_ids.discard(object_id)

    def _should_drop_server_msg(self, obj_id, opcode, body):
        if obj_id == 1 and opcode == 1:
            self._forget(body)
        elif obj_id in self._registry_ids and opcode == 0:
            if len(body) < 4:
                raise ProtocolError("wl_registry.global truncated")
            name = struct.unpack_from("<I", body, 0)[0]
            interface, off = self._parse_string(body, 4)
            if off + 4 > len(body):
                raise ProtocolError("wl_registry.global truncated after interface")
            self._globals[name] = interface
            self._check_capacity()
            if self.mode == "hide-layer-shell" and interface == LAYER_SHELL:
                self.dropped += 1
                return True
        elif obj_id in self._layer_surface_ids and opcode == 0:
            if self.mode == "drop-layer-configure":
                self.dropped += 1
                return True
        return False

    def from_server(self, data: bytes) -> bytes:
        """Buffers server bytes and applies configured drop rules to complete messages."""
        self._server_buf.extend(data)
        if len(self._server_buf) > MAX_BUFFER:
            raise ProtocolError("server buffer overflow")
        out = bytearray()
        for _ in range(MAX_MSGS_PER_CHUNK):
            if len(self._server_buf) < 8:
                break
            size = struct.unpack_from("<I", self._server_buf, 4)[0] >> 16
            if size < 8 or size % 4 != 0:
                raise ProtocolError(f"invalid server message size: {size}")
            if len(self._server_buf) < size:
                break
            msg = bytes(self._server_buf[:size])
            del self._server_buf[:size]
            obj_id = struct.unpack_from("<I", msg, 0)[0]
            opcode = struct.unpack_from("<I", msg, 4)[0] & 0xFFFF
            if not self._should_drop_server_msg(obj_id, opcode, msg[8:]):
                out.extend(msg)
        return bytes(out)


def _recv_with_fds(sock):
    """Returns (data, fds), or None when the read would block; (b"", []) is end of stream."""
    cmsg_space = socket.CMSG_SPACE(28 * struct.calcsize("i"))
    try:
        data, ancdata, _flags, _addr = sock.recvmsg(4096, cmsg_space)
    except (BlockingIOError, InterruptedError):
        return None
    fds = []
    for cmsg_level, cmsg_type, cmsg_data in ancdata:
        if cmsg_level == socket.SOL_SOCKET and cmsg_type == socket.SCM_RIGHTS:
            count = len(cmsg_data) // struct.calcsize("i")
            fds.extend(struct.unpack(f"<{count}i", cmsg_data[: count * 4]))
    return data, fds


def _send_with_fds(sock, pending_bytes, pending_fds):
    if not pending_bytes:
        return
    ancdata = []
    sent_fds = []
    if pending_fds:
        to_send = pending_fds[:28]
        sent_fds = to_send
        ancdata = [
            (
                socket.SOL_SOCKET,
                socket.SCM_RIGHTS,
                struct.pack(f"<{len(to_send)}i", *to_send),
            )
        ]
    try:
        sent = sock.sendmsg([pending_bytes], ancdata)
    except (BlockingIOError, InterruptedError):
        return
    del pending_bytes[:sent]
    if sent_fds:
        del pending_fds[: len(sent_fds)]
        for fd in sent_fds:
            try:
                os.close(fd)
            except OSError:
                pass


def _close_fds(fds):
    for fd in fds:
        try:
            os.close(fd)
        except OSError:
            pass


def _handle_client_read(client_sock, server_sock, filt, c2s_bytes, c2s_fds):
    received = _recv_with_fds(client_sock)
    if received is None:
        return True
    data, fds = received
    if not data and not fds:
        return False
    c2s_bytes.extend(filt.from_client(data))
    c2s_fds.extend(fds)
    _send_with_fds(server_sock, c2s_bytes, c2s_fds)
    return True


def _handle_server_read(client_sock, server_sock, filt, s2c_bytes, s2c_fds):
    received = _recv_with_fds(server_sock)
    if received is None:
        return True
    data, fds = received
    if not data and not fds:
        return False
    s2c_bytes.extend(filt.from_server(data))
    s2c_fds.extend(fds)
    _send_with_fds(client_sock, s2c_bytes, s2c_fds)
    return True


def _flush_pending(directions):
    """Retries every direction's unsent bytes, so a partial send never waits for a new read."""
    for sock, pending_bytes, pending_fds in directions:
        _send_with_fds(sock, pending_bytes, pending_fds)


def relay(client_sock, server_sock, filt, deadline_s):
    """Relays Wayland traffic between client and server under deadline and transforms."""
    client_sock.setblocking(False)
    server_sock.setblocking(False)
    sel = selectors.DefaultSelector()
    sel.register(client_sock, selectors.EVENT_READ, "client")
    sel.register(server_sock, selectors.EVENT_READ, "server")

    c2s_bytes = bytearray()
    c2s_fds = []
    s2c_bytes = bytearray()
    s2c_fds = []
    start = time.monotonic()

    try:
        for _ in range(MAX_RELAY_ITERATIONS):
            remaining = deadline_s - (time.monotonic() - start)
            if remaining <= 0:
                break
            _flush_pending(((server_sock, c2s_bytes, c2s_fds), (client_sock, s2c_bytes, s2c_fds)))
            events = sel.select(timeout=min(remaining, 0.2))
            active = True
            for key, _mask in events:
                if key.data == "client":
                    if not _handle_client_read(client_sock, server_sock, filt, c2s_bytes, c2s_fds):
                        active = False
                        break
                elif key.data == "server":
                    if not _handle_server_read(client_sock, server_sock, filt, s2c_bytes, s2c_fds):
                        active = False
                        break
            if not active:
                break
    finally:
        _close_fds(c2s_fds)
        _close_fds(s2c_fds)
        sel.close()
        client_sock.close()
        server_sock.close()


def _run_relay_thread(client_sock, upstream_path, mode, deadline_s):
    try:
        server_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server_sock.connect(upstream_path)
    except OSError:
        client_sock.close()
        return
    filt = Filter(mode)
    try:
        relay(client_sock, server_sock, filt, deadline_s)
    except ProtocolError as error:
        # The relay has closed both sockets; the client sees the end of the stream.
        sys.stderr.write(f"wayland_filter: {mode}: {error}\n")


def serve(listen_path, upstream_path, mode, seconds, max_clients=8):
    """Runs the AF_UNIX listener and dispatches client relay threads."""
    if os.path.exists(listen_path):
        try:
            os.unlink(listen_path)
        except OSError:
            pass
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.setblocking(False)
    listener.bind(listen_path)
    listener.listen(max_clients)

    start = time.monotonic()
    client_count = 0
    try:
        sys.stdout.write(f"READY {listen_path}\n")
        sys.stdout.flush()
        for _ in range(100_000):
            remaining = seconds - (time.monotonic() - start)
            if remaining <= 0 or client_count >= max_clients:
                break
            r, _, _ = select.select([listener], [], [], min(remaining, 0.2))
            if not r:
                continue
            try:
                client_sock, _ = listener.accept()
            except (BlockingIOError, InterruptedError):
                continue
            client_count += 1
            thread = threading.Thread(
                target=_run_relay_thread,
                args=(client_sock, upstream_path, mode, remaining),
                daemon=True,
            )
            thread.start()
    finally:
        listener.close()
        if os.path.exists(listen_path):
            try:
                os.unlink(listen_path)
            except OSError:
                pass


def parse_args(argv=None):
    """Parses and validates CLI arguments."""
    parser = argparse.ArgumentParser(description="Wayland protocol filter proxy")
    parser.add_argument("--listen", required=True, help="AF_UNIX listening socket path")
    parser.add_argument("--upstream", required=True, help="Upstream compositor socket path")
    parser.add_argument("--mode", required=True, choices=MODES, help="Filter mode")
    parser.add_argument("--seconds", type=int, default=60, help="Run duration in seconds (1..600)")
    args = parser.parse_args(argv)
    if not (1 <= args.seconds <= 600):
        parser.error("--seconds must be between 1 and 600")
    return args


def main(argv=None):
    """Entry point for the wayland_filter CLI."""
    if not (hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS")):
        sys.stdout.write("SKIP: AF_UNIX and SCM_RIGHTS are POSIX-only; filter unavailable\n")
        sys.stdout.flush()
        return 2

    args = parse_args(argv)
    serve(args.listen, args.upstream, args.mode, args.seconds)
    return 0


if __name__ == "__main__":
    sys.exit(main())
