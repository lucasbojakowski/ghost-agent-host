#define PY_SSIZE_T_CLEAN
#include <Python.h>

#ifndef _WIN32
#error "ghost_native is currently Windows-only"
#endif

#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#include <limits.h>

#pragma comment(lib, "Ws2_32.lib")

#define GHOST_NATIVE_API_VERSION 1
#define GHOST_PHASE_DISCONNECTED 0
#define GHOST_PHASE_CONNECTING 1
#define GHOST_PHASE_CONNECTED 2
#define GHOST_MAX_IO_BYTES (1024 * 1024)

typedef struct {
    SOCKET sock;
    int phase;
    int last_error;
    int wsa_started;
    int winsock_version;
    int winsock_high_version;
} GhostNativeState;

static GhostNativeState *
ghost_state(PyObject *module)
{
    return (GhostNativeState *)PyModule_GetState(module);
}

static const char *
ghost_phase_name(int phase)
{
    switch (phase) {
        case GHOST_PHASE_CONNECTING:
            return "connecting";
        case GHOST_PHASE_CONNECTED:
            return "connected";
        default:
            return "disconnected";
    }
}

static void
ghost_close_socket(GhostNativeState *state)
{
    if (state != NULL && state->sock != INVALID_SOCKET) {
        closesocket(state->sock);
        state->sock = INVALID_SOCKET;
    }
    if (state != NULL) {
        state->phase = GHOST_PHASE_DISCONNECTED;
    }
}

static PyObject *
ghost_raise_wsa(GhostNativeState *state, int error)
{
    if (state != NULL) {
        state->last_error = error;
        ghost_close_socket(state);
    }
    return PyErr_SetExcFromWindowsErr(PyExc_OSError, error);
}

static PyObject *
ghost_error_result(const char *stage, int winerror)
{
    return Py_BuildValue(
        "{s:O,s:s,s:i}",
        "ok", Py_False,
        "stage", stage,
        "winerror", winerror);
}

static PyObject *
ghost_ping(PyObject *self, PyObject *Py_UNUSED(args))
{
    (void)self;
    return PyUnicode_FromString("ghost-native-ok");
}

static PyObject *
ghost_add(PyObject *self, PyObject *args)
{
    long a;
    long b;

    (void)self;
    if (!PyArg_ParseTuple(args, "ll:add", &a, &b)) {
        return NULL;
    }
    return PyLong_FromLong(a + b);
}

static PyObject *
ghost_pid(PyObject *self, PyObject *Py_UNUSED(args))
{
    (void)self;
    return PyLong_FromUnsignedLong(GetCurrentProcessId());
}

static PyObject *
ghost_status(PyObject *self, PyObject *Py_UNUSED(args))
{
    GhostNativeState *state = ghost_state(self);
    if (state == NULL) {
        return NULL;
    }
    return Py_BuildValue(
        "{s:s,s:i}",
        "status", ghost_phase_name(state->phase),
        "winerror", state->last_error);
}

static PyObject *
ghost_close(PyObject *self, PyObject *Py_UNUSED(args))
{
    GhostNativeState *state = ghost_state(self);
    if (state == NULL) {
        return NULL;
    }
    ghost_close_socket(state);
    state->last_error = 0;
    Py_RETURN_NONE;
}

static PyObject *
ghost_start(PyObject *self, PyObject *args)
{
    GhostNativeState *state = ghost_state(self);
    const char *host = "127.0.0.1";
    int port = 48766;
    struct sockaddr_in address;
    u_long nonblocking = 1;
    int result;
    int error;

    if (state == NULL) {
        return NULL;
    }
    if (!PyArg_ParseTuple(args, "|si:start", &host, &port)) {
        return NULL;
    }
    if (port < 1 || port > 65535) {
        PyErr_SetString(PyExc_ValueError, "port must be between 1 and 65535");
        return NULL;
    }
    if (!state->wsa_started) {
        PyErr_SetString(PyExc_RuntimeError, "WinSock is not initialized");
        return NULL;
    }

    ghost_close_socket(state);
    state->last_error = 0;

    state->sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (state->sock == INVALID_SOCKET) {
        return ghost_raise_wsa(state, WSAGetLastError());
    }

    result = ioctlsocket(state->sock, FIONBIO, &nonblocking);
    if (result == SOCKET_ERROR) {
        return ghost_raise_wsa(state, WSAGetLastError());
    }

    ZeroMemory(&address, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_port = htons((u_short)port);
    result = InetPtonA(AF_INET, host, &address.sin_addr);
    if (result != 1) {
        error = result == 0 ? WSAEINVAL : WSAGetLastError();
        return ghost_raise_wsa(state, error);
    }

    result = connect(
        state->sock,
        (const struct sockaddr *)&address,
        (int)sizeof(address));
    if (result == 0) {
        state->phase = GHOST_PHASE_CONNECTED;
        return PyUnicode_FromString("connected");
    }

    error = WSAGetLastError();
    if (error == WSAEWOULDBLOCK || error == WSAEINPROGRESS || error == WSAEALREADY) {
        state->phase = GHOST_PHASE_CONNECTING;
        return PyUnicode_FromString("connecting");
    }
    if (error == WSAEISCONN) {
        state->phase = GHOST_PHASE_CONNECTED;
        return PyUnicode_FromString("connected");
    }

    return ghost_raise_wsa(state, error);
}

static PyObject *
ghost_poll(PyObject *self, PyObject *Py_UNUSED(args))
{
    GhostNativeState *state = ghost_state(self);
    fd_set write_set;
    fd_set error_set;
    struct timeval timeout;
    int result;
    int socket_error = 0;
    int socket_error_length = (int)sizeof(socket_error);

    if (state == NULL) {
        return NULL;
    }
    if (state->phase != GHOST_PHASE_CONNECTING) {
        return PyUnicode_FromString(ghost_phase_name(state->phase));
    }

    FD_ZERO(&write_set);
    FD_ZERO(&error_set);
    FD_SET(state->sock, &write_set);
    FD_SET(state->sock, &error_set);
    timeout.tv_sec = 0;
    timeout.tv_usec = 0;

    result = select(0, NULL, &write_set, &error_set, &timeout);
    if (result == SOCKET_ERROR) {
        return ghost_raise_wsa(state, WSAGetLastError());
    }
    if (result == 0) {
        return PyUnicode_FromString("connecting");
    }

    result = getsockopt(
        state->sock,
        SOL_SOCKET,
        SO_ERROR,
        (char *)&socket_error,
        &socket_error_length);
    if (result == SOCKET_ERROR) {
        return ghost_raise_wsa(state, WSAGetLastError());
    }
    if (socket_error != 0) {
        return ghost_raise_wsa(state, socket_error);
    }

    state->phase = GHOST_PHASE_CONNECTED;
    state->last_error = 0;
    return PyUnicode_FromString("connected");
}

static PyObject *
ghost_recv(PyObject *self, PyObject *args)
{
    GhostNativeState *state = ghost_state(self);
    Py_ssize_t max_bytes = 4096;
    char *buffer;
    int result;
    int error;
    PyObject *bytes;

    if (state == NULL) {
        return NULL;
    }
    if (!PyArg_ParseTuple(args, "|n:recv", &max_bytes)) {
        return NULL;
    }
    if (max_bytes < 1 || max_bytes > GHOST_MAX_IO_BYTES || max_bytes > INT_MAX) {
        PyErr_Format(
            PyExc_ValueError,
            "max_bytes must be between 1 and %d",
            GHOST_MAX_IO_BYTES);
        return NULL;
    }
    if (state->phase != GHOST_PHASE_CONNECTED) {
        Py_RETURN_NONE;
    }

    buffer = (char *)PyMem_Malloc((size_t)max_bytes);
    if (buffer == NULL) {
        return PyErr_NoMemory();
    }

    result = recv(state->sock, buffer, (int)max_bytes, 0);
    if (result > 0) {
        bytes = PyBytes_FromStringAndSize(buffer, result);
        PyMem_Free(buffer);
        return bytes;
    }
    PyMem_Free(buffer);

    if (result == 0) {
        ghost_close_socket(state);
        state->last_error = 0;
        return PyBytes_FromStringAndSize("", 0);
    }

    error = WSAGetLastError();
    if (error == WSAEWOULDBLOCK) {
        Py_RETURN_NONE;
    }
    return ghost_raise_wsa(state, error);
}

static PyObject *
ghost_send(PyObject *self, PyObject *args)
{
    GhostNativeState *state = ghost_state(self);
    const char *buffer;
    Py_ssize_t length;
    int to_send;
    int result;
    int error;

    if (state == NULL) {
        return NULL;
    }
    if (!PyArg_ParseTuple(args, "y#:send", &buffer, &length)) {
        return NULL;
    }
    if (state->phase != GHOST_PHASE_CONNECTED || length == 0) {
        return PyLong_FromLong(0);
    }

    to_send = length > INT_MAX ? INT_MAX : (int)length;
    result = send(state->sock, buffer, to_send, 0);
    if (result >= 0) {
        return PyLong_FromLong(result);
    }

    error = WSAGetLastError();
    if (error == WSAEWOULDBLOCK) {
        return PyLong_FromLong(0);
    }
    return ghost_raise_wsa(state, error);
}

static PyObject *
ghost_socket_probe(PyObject *self, PyObject *Py_UNUSED(args))
{
    GhostNativeState *state = ghost_state(self);
    SOCKET sock = INVALID_SOCKET;
    u_long nonblocking = 1;
    int result;
    int error;

    if (state == NULL) {
        return NULL;
    }
    if (!state->wsa_started) {
        return ghost_error_result("WSAStartup", WSANOTINITIALISED);
    }

    sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) {
        return ghost_error_result("socket", WSAGetLastError());
    }

    result = ioctlsocket(sock, FIONBIO, &nonblocking);
    if (result == SOCKET_ERROR) {
        error = WSAGetLastError();
        closesocket(sock);
        return ghost_error_result("ioctlsocket", error);
    }

    closesocket(sock);

    return Py_BuildValue(
        "{s:O,s:s,s:i,s:i,s:O}",
        "ok", Py_True,
        "stage", "complete",
        "winsock_version", state->winsock_version,
        "winsock_high_version", state->winsock_high_version,
        "nonblocking", Py_True);
}

static PyObject *
ghost_connect_probe(PyObject *self, PyObject *args)
{
    GhostNativeState *state = ghost_state(self);
    const char *host = "127.0.0.1";
    int port = 48766;
    SOCKET sock = INVALID_SOCKET;
    struct sockaddr_in address;
    u_long nonblocking = 1;
    int result;
    int error;
    const char *status;

    if (state == NULL) {
        return NULL;
    }
    if (!PyArg_ParseTuple(args, "|si:connect_probe", &host, &port)) {
        return NULL;
    }
    if (port < 1 || port > 65535) {
        PyErr_SetString(PyExc_ValueError, "port must be between 1 and 65535");
        return NULL;
    }
    if (!state->wsa_started) {
        return ghost_error_result("WSAStartup", WSANOTINITIALISED);
    }

    sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) {
        return ghost_error_result("socket", WSAGetLastError());
    }

    result = ioctlsocket(sock, FIONBIO, &nonblocking);
    if (result == SOCKET_ERROR) {
        error = WSAGetLastError();
        closesocket(sock);
        return ghost_error_result("ioctlsocket", error);
    }

    ZeroMemory(&address, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_port = htons((u_short)port);
    result = InetPtonA(AF_INET, host, &address.sin_addr);
    if (result != 1) {
        error = result == 0 ? WSAEINVAL : WSAGetLastError();
        closesocket(sock);
        return ghost_error_result("InetPtonA", error);
    }

    result = connect(sock, (const struct sockaddr *)&address, (int)sizeof(address));
    if (result == 0) {
        status = "connected";
        error = 0;
    } else {
        error = WSAGetLastError();
        if (error == WSAEWOULDBLOCK || error == WSAEINPROGRESS || error == WSAEALREADY) {
            status = "in_progress";
        } else {
            closesocket(sock);
            return ghost_error_result("connect", error);
        }
    }

    closesocket(sock);

    return Py_BuildValue(
        "{s:O,s:s,s:s,s:i,s:i}",
        "ok", Py_True,
        "stage", "connect",
        "status", status,
        "winerror", error,
        "port", port);
}

static PyMethodDef ghost_methods[] = {
    {"ping", ghost_ping, METH_NOARGS, "Return a native module health marker."},
    {"add", ghost_add, METH_VARARGS, "Add two integers."},
    {"pid", ghost_pid, METH_NOARGS, "Return the FL Studio process id."},
    {"status", ghost_status, METH_NOARGS, "Return native transport status."},
    {"start", ghost_start, METH_VARARGS, "Start a native nonblocking IPv4 TCP connection."},
    {"poll", ghost_poll, METH_NOARGS, "Advance a pending nonblocking connection without waiting."},
    {"recv", ghost_recv, METH_VARARGS, "Receive up to max_bytes without blocking."},
    {"send", ghost_send, METH_VARARGS, "Send bytes without blocking."},
    {"close", ghost_close, METH_NOARGS, "Close the native transport socket."},
    {"socket_probe", ghost_socket_probe, METH_NOARGS,
     "Create a native nonblocking WinSock TCP socket and close it."},
    {"connect_probe", ghost_connect_probe, METH_VARARGS,
     "Start a native nonblocking IPv4 TCP connection and immediately close it."},
    {NULL, NULL, 0, NULL}
};

static int
ghost_exec(PyObject *module)
{
    GhostNativeState *state = ghost_state(module);
    WSADATA data;
    int result;

    if (state == NULL) {
        return -1;
    }

    state->sock = INVALID_SOCKET;
    state->phase = GHOST_PHASE_DISCONNECTED;
    state->last_error = 0;
    state->wsa_started = 0;
    state->winsock_version = 0;
    state->winsock_high_version = 0;

    result = WSAStartup(MAKEWORD(2, 2), &data);
    if (result != 0) {
        PyErr_Format(PyExc_OSError, "WSAStartup failed with WinSock error %d", result);
        return -1;
    }

    state->wsa_started = 1;
    state->winsock_version = (int)data.wVersion;
    state->winsock_high_version = (int)data.wHighVersion;

    if (PyModule_AddIntConstant(module, "API_VERSION", GHOST_NATIVE_API_VERSION) < 0) {
        return -1;
    }
    if (PyModule_AddStringConstant(module, "runtime", "cp312-subinterpreter-native-transport") < 0) {
        return -1;
    }
    return 0;
}

static void
ghost_free(void *module)
{
    GhostNativeState *state = ghost_state((PyObject *)module);
    if (state == NULL) {
        return;
    }
    ghost_close_socket(state);
    if (state->wsa_started) {
        WSACleanup();
        state->wsa_started = 0;
    }
}

static PyModuleDef_Slot ghost_slots[] = {
    {Py_mod_exec, ghost_exec},
    {Py_mod_multiple_interpreters, Py_MOD_PER_INTERPRETER_GIL_SUPPORTED},
    {0, NULL}
};

static struct PyModuleDef ghost_module = {
    PyModuleDef_HEAD_INIT,
    "ghost_native",
    "Ghost & Guild FL Studio native transport.",
    sizeof(GhostNativeState),
    ghost_methods,
    ghost_slots,
    NULL,
    NULL,
    ghost_free
};

PyMODINIT_FUNC
PyInit_ghost_native(void)
{
    return PyModuleDef_Init(&ghost_module);
}
