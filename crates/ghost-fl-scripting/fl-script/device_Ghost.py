# name=Ghost Bridge
# supportedDevices=Ghost Midi

import json
import os
import time

try:
    import ghost_native
except Exception as error:
    ghost_native = None
    _NATIVE_IMPORT_ERROR = error
else:
    _NATIVE_IMPORT_ERROR = None

import arrangement
import channels
import device
import general
import launchMapPages
import mixer
import patterns
import playlist
import plugins
import transport
import ui

PROTOCOL_VERSION = 1
NATIVE_API_VERSION = 1
BRIDGE_NAME = "ghost-fl-scripting"
BRIDGE_HOST = os.environ.get("GHOST_FL_SCRIPTING_HOST", "127.0.0.1")
BRIDGE_PORT = int(os.environ.get("GHOST_FL_SCRIPTING_PORT", "48766"))

MAX_FRAME_BYTES = 64 * 1024
MAX_BUFFER_BYTES = 256 * 1024
MAX_READS_PER_IDLE = 4
MAX_CALLS_PER_IDLE = 8
MAX_WRITES_PER_IDLE = 4
IO_CHUNK_BYTES = 4096
MIN_RECONNECT_SECONDS = 0.25
MAX_RECONNECT_SECONDS = 5.0
ERROR_REPEAT_SECONDS = 15.0

ALLOWED_MODULES = {
    "arrangement": arrangement,
    "channels": channels,
    "device": device,
    "general": general,
    "launchMapPages": launchMapPages,
    "mixer": mixer,
    "patterns": patterns,
    "playlist": playlist,
    "plugins": plugins,
    "transport": transport,
    "ui": ui,
}

_connecting = False
_connected = False
_receive_buffer = bytearray()
_send_buffer = bytearray()
_next_connect_at = 0.0
_reconnect_delay = MIN_RECONNECT_SECONDS
_last_transport_error = None
_last_transport_error_at = 0.0


def OnInit():
    _reset_transport(schedule_reconnect=False)
    if not _native_transport_available():
        _report_native_unavailable()
        return
    _try_connect()


def OnDeInit():
    _reset_transport(schedule_reconnect=False)


def OnIdle():
    if not _native_transport_available():
        _report_native_unavailable()
        return

    if not _connected:
        _try_connect()
        _finish_connect_if_ready()
        return

    _read_bounded()
    _dispatch_bounded()
    _write_bounded()


def _native_transport_available():
    return ghost_native is not None and getattr(ghost_native, "API_VERSION", None) == NATIVE_API_VERSION


def _report_native_unavailable():
    if ghost_native is None:
        error = _NATIVE_IMPORT_ERROR or RuntimeError("ghost_native could not be imported")
        _report_transport_error("native transport unavailable", error)
        return
    _report_transport_error(
        "native transport unavailable",
        RuntimeError(
            "ghost_native API version "
            + repr(getattr(ghost_native, "API_VERSION", None))
            + " does not match required version "
            + str(NATIVE_API_VERSION)
        ),
    )


def _try_connect():
    global _connecting
    if _connecting or _connected or time.monotonic() < _next_connect_at:
        return

    try:
        status = ghost_native.start(BRIDGE_HOST, BRIDGE_PORT)
    except Exception as error:
        _report_transport_error("native connect start failed", error)
        _schedule_reconnect()
        return

    if status == "connected":
        _mark_connected()
        return
    if status == "connecting":
        _connecting = True
        return

    _report_transport_error("native connect start failed", RuntimeError("unexpected status: " + repr(status)))
    _reset_transport(schedule_reconnect=True)


def _finish_connect_if_ready():
    global _connecting
    if not _connecting or _connected:
        return

    try:
        status = ghost_native.poll()
    except Exception as error:
        _report_transport_error("native connect completion failed", error)
        _reset_transport(schedule_reconnect=True)
        return

    if status == "connected":
        _connecting = False
        _mark_connected()
    elif status == "connecting":
        return
    else:
        _report_transport_error(
            "native connect completion failed",
            RuntimeError("unexpected status: " + repr(status)),
        )
        _reset_transport(schedule_reconnect=True)


def _mark_connected():
    global _connected, _connecting, _reconnect_delay
    global _last_transport_error, _last_transport_error_at
    _connected = True
    _connecting = False
    _reconnect_delay = MIN_RECONNECT_SECONDS
    _last_transport_error = None
    _last_transport_error_at = 0.0
    _queue_message(
        {
            "type": "hello",
            "protocol": PROTOCOL_VERSION,
            "bridge": BRIDGE_NAME,
            "fl_version": _safe_fl_version(),
            "scripting_api_version": _safe_api_version(),
        }
    )
    _write_bounded()


def _safe_fl_version():
    try:
        return ui.getVersion(5)
    except Exception:
        try:
            return ui.getVersion()
        except Exception:
            return None


def _safe_api_version():
    try:
        return general.getVersion()
    except Exception:
        return None


def _read_bounded():
    for _ in range(MAX_READS_PER_IDLE):
        try:
            chunk = ghost_native.recv(IO_CHUNK_BYTES)
        except Exception as error:
            _report_transport_error("native socket read failed", error)
            _reset_transport(schedule_reconnect=True)
            return

        if chunk is None:
            return
        if not chunk:
            _report_transport_error("native socket read failed", EOFError("bridge closed the connection"))
            _reset_transport(schedule_reconnect=True)
            return

        if len(_receive_buffer) + len(chunk) > MAX_BUFFER_BYTES:
            _receive_buffer.clear()
            _report_transport_error(
                "native socket read failed",
                BufferError("receive buffer exceeded bridge limit"),
            )
            _reset_transport(schedule_reconnect=True)
            return
        _receive_buffer.extend(chunk)


def _dispatch_bounded():
    for _ in range(MAX_CALLS_PER_IDLE):
        newline = _receive_buffer.find(b"\n")
        if newline < 0:
            if len(_receive_buffer) > MAX_FRAME_BYTES:
                _receive_buffer.clear()
            return

        frame = bytes(_receive_buffer[:newline])
        del _receive_buffer[: newline + 1]
        if frame.endswith(b"\r"):
            frame = frame[:-1]
        if not frame or len(frame) > MAX_FRAME_BYTES:
            continue

        try:
            message = json.loads(frame.decode("utf-8"))
        except Exception:
            continue
        _handle_message(message)


def _handle_message(message):
    request_id = message.get("id") if isinstance(message, dict) else None
    if not isinstance(message, dict) or message.get("type") != "call":
        _queue_error(request_id, "invalid_request", "expected a call object")
        return

    module_name = message.get("module")
    function_name = message.get("function")
    args = message.get("args", [])

    module = ALLOWED_MODULES.get(module_name)
    if module is None:
        _queue_error(request_id, "module_not_allowed", "module is not allowlisted")
        return
    if not _safe_identifier(function_name):
        _queue_error(request_id, "invalid_function", "function name is not a safe identifier")
        return
    if not isinstance(args, list):
        _queue_error(request_id, "invalid_args", "args must be a JSON array")
        return

    function = getattr(module, function_name, None)
    if function is None or not callable(function):
        _queue_error(request_id, "function_not_found", "requested member is not callable")
        return

    try:
        value = function(*args)
        value = _to_json_value(value)
    except Exception as error:
        _queue_error(request_id, "call_failed", str(error))
        return

    _queue_message({"type": "result", "id": request_id, "ok": True, "value": value})


def _safe_identifier(value):
    if not isinstance(value, str) or not value or value.startswith("_"):
        return False
    first = value[0]
    if not (first.isalpha() and first.isascii()):
        return False
    for character in value[1:]:
        if not ((character.isalnum() and character.isascii()) or character == "_"):
            return False
    return True


def _to_json_value(value, depth=0):
    if depth > 8:
        raise TypeError("return value nesting exceeded bridge limit")
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, (list, tuple)):
        return [_to_json_value(item, depth + 1) for item in value]
    if isinstance(value, dict):
        result = {}
        for key, item in value.items():
            if not isinstance(key, (str, int, float, bool)):
                raise TypeError("return dictionary contains an unsupported key type")
            result[str(key)] = _to_json_value(item, depth + 1)
        return result
    raise TypeError("unsupported return type: " + type(value).__name__)


def _queue_error(request_id, kind, message):
    _queue_message(
        {
            "type": "result",
            "id": request_id,
            "ok": False,
            "error": {"kind": kind, "message": message},
        }
    )


def _queue_message(message):
    try:
        frame = json.dumps(message, separators=(",", ":"), ensure_ascii=False).encode("utf-8") + b"\n"
    except Exception:
        return

    if len(frame) > MAX_FRAME_BYTES:
        return
    if len(_send_buffer) + len(frame) > MAX_BUFFER_BYTES:
        _reset_transport(schedule_reconnect=True)
        return
    _send_buffer.extend(frame)


def _write_bounded():
    for _ in range(MAX_WRITES_PER_IDLE):
        if not _send_buffer:
            return
        chunk = bytes(_send_buffer[:IO_CHUNK_BYTES])
        try:
            count = ghost_native.send(chunk)
        except Exception as error:
            _report_transport_error("native socket write failed", error)
            _reset_transport(schedule_reconnect=True)
            return
        if count == 0:
            return
        if count < 0 or count > len(chunk):
            _report_transport_error(
                "native socket write failed",
                OSError("native send returned an invalid byte count"),
            )
            _reset_transport(schedule_reconnect=True)
            return
        del _send_buffer[:count]


def _report_transport_error(context, error):
    global _last_transport_error, _last_transport_error_at
    now = time.monotonic()
    message = context + ": " + repr(error)
    if message != _last_transport_error or now - _last_transport_error_at >= ERROR_REPEAT_SECONDS:
        print("[Ghost Bridge] " + message)
        _last_transport_error = message
        _last_transport_error_at = now


def _reset_transport(schedule_reconnect):
    global _connecting, _connected, _next_connect_at, _reconnect_delay
    if ghost_native is not None:
        try:
            ghost_native.close()
        except Exception:
            pass
    _connecting = False
    _connected = False
    _receive_buffer.clear()
    _send_buffer.clear()
    if schedule_reconnect:
        _schedule_reconnect()
    else:
        _next_connect_at = 0.0
        _reconnect_delay = MIN_RECONNECT_SECONDS


def _schedule_reconnect():
    global _next_connect_at, _reconnect_delay
    _next_connect_at = time.monotonic() + _reconnect_delay
    _reconnect_delay = min(MAX_RECONNECT_SECONDS, _reconnect_delay * 2.0)
