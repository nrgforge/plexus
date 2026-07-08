"""Minimal MCP stdio client for driving a plexus subprocess.

Speaks newline-delimited JSON-RPC 2.0 over stdin/stdout, matching
rmcp's stdio transport. Stdlib only — no external dependencies.

One client = one subprocess = one consumer process. Scenarios that
need the multi-process story spawn two clients against the same DB.
"""

import json
import queue
import subprocess
import threading


class McpError(RuntimeError):
    """Tool call returned isError or the transport failed."""


class McpClient:
    def __init__(self, argv, cwd=None, stderr_path=None):
        self._stderr = open(stderr_path, "ab") if stderr_path else subprocess.DEVNULL
        self.proc = subprocess.Popen(
            argv,
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self._stderr,
        )
        self._responses = queue.Queue()
        self._next_id = 0
        threading.Thread(target=self._read_loop, daemon=True).start()
        self._initialize()

    def _read_loop(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "id" in msg and ("result" in msg or "error" in msg):
                self._responses.put(msg)

    def _send(self, obj):
        self.proc.stdin.write((json.dumps(obj) + "\n").encode())
        self.proc.stdin.flush()

    def _request(self, method, params, timeout=60):
        self._next_id += 1
        req_id = self._next_id
        self._send({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params})
        while True:
            try:
                msg = self._responses.get(timeout=timeout)
            except queue.Empty:
                raise McpError(f"timeout waiting for response to {method} (id={req_id})")
            if msg.get("id") != req_id:
                continue
            if "error" in msg:
                raise McpError(f"{method} failed: {msg['error']}")
            return msg["result"]

    def _initialize(self):
        self._request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "play-harness", "version": "0.1"},
            },
        )
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def call(self, tool, arguments=None, timeout=120):
        """Call a tool; return parsed JSON of the first text content.

        Falls back to the raw text when the tool returns non-JSON text.
        Raises McpError when the tool reports isError.
        """
        result = self._request(
            "tools/call", {"name": tool, "arguments": arguments or {}}, timeout=timeout
        )
        texts = [c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"]
        text = "\n".join(texts)
        if result.get("isError"):
            raise McpError(f"{tool}: {text}")
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()
        if self._stderr is not subprocess.DEVNULL:
            self._stderr.close()
