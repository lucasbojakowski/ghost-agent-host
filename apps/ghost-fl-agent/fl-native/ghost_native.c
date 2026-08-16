#define PY_SSIZE_T_CLEAN
#include <Python.h>

#ifndef _WIN32
#error "ghost_native is currently Windows-only"
#endif

#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#pragma comment(lib, "Ws2_32.lib")

#define GHOST_NATIVE_API_VERSION 1

typedef struct {
    int reserved;
} GhostNativeState;

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
ghost_socket_probe(PyObject *self, PyObject *Py_UNUSED(args))
{
    WSADATA data;
    SOCKET sock = INVALID_SOCKET;
    u_long nonblocking = 1;
    int result;
    int error;

    (void)self;

    result = WSAStartup(MAKEWORD(2, 2), &data);
    if (result != 0) {
        return ghost_error_result("WSAStartup", result);
    }

    sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) {
        error = WSAGetLastError();
        WSACleanup();
        return ghost_error_result("socket", error);
    }

    result = ioctlsocket(sock, FIONBIO, &nonblocking);
    if (result == SOCKET_ERROR) {
        error = WSAGetLastError();
        closesocket(sock);
        WSACleanup();
        return ghost_error_result("ioctlsocket", error);
    }

    closesocket(sock);
    WSACleanup();

    return Py_BuildValue(
        "{s:O,s:s,s:i,s:i,s:O}",
        "ok", Py_True,
        "stage", "complete",
        "winsock_version", (int)data.wVersion,
        "winsock_high_version", (int)data.wHighVersion,
        "nonblocking", Py_True);
}

static PyObject *
ghost_connect_probe(PyObject *self, PyObject *args)
{
    const char *host = "127.0.0.1";
    int port = 48766;
    WSADATA data;
    SOCKET sock = INVALID_SOCKET;
    struct sockaddr_in address;
    u_long nonblocking = 1;
    int result;
    int error;
    const char *status;

    (void)self;

    if (!PyArg_ParseTuple(args, "|si:connect_probe", &host, &port)) {
        return NULL;
    }
    if (port < 1 || port > 65535) {
        PyErr_SetString(PyExc_ValueError, "port must be between 1 and 65535");
        return NULL;
    }

    result = WSAStartup(MAKEWORD(2, 2), &data);
    if (result != 0) {
        return ghost_error_result("WSAStartup", result);
    }

    sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) {
        error = WSAGetLastError();
        WSACleanup();
        return ghost_error_result("socket", error);
    }

    result = ioctlsocket(sock, FIONBIO, &nonblocking);
    if (result == SOCKET_ERROR) {
        error = WSAGetLastError();
        closesocket(sock);
        WSACleanup();
        return ghost_error_result("ioctlsocket", error);
    }

    ZeroMemory(&address, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_port = htons((u_short)port);
    result = InetPtonA(AF_INET, host, &address.sin_addr);
    if (result != 1) {
        error = result == 0 ? WSAEINVAL : WSAGetLastError();
        closesocket(sock);
        WSACleanup();
        return ghost_error_result("InetPtonA", error);
    }

    result = connect(sock, (const struct sockaddr *)&address, sizeof(address));
    if (result == 0) {
        status = "connected";
        error = 0;
    } else {
        error = WSAGetLastError();
        if (error == WSAEWOULDBLOCK || error == WSAEINPROGRESS || error == WSAEALREADY) {
            status = "in_progress";
        } else {
            closesocket(sock);
            WSACleanup();
            return ghost_error_result("connect", error);
        }
    }

    closesocket(sock);
    WSACleanup();

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
    {"socket_probe", ghost_socket_probe, METH_NOARGS,
     "Create a native nonblocking WinSock TCP socket and close it."},
    {"connect_probe", ghost_connect_probe, METH_VARARGS,
     "Start a native nonblocking IPv4 TCP connection and immediately close it."},
    {NULL, NULL, 0, NULL}
};

static int
ghost_exec(PyObject *module)
{
    if (PyModule_AddIntConstant(module, "API_VERSION", GHOST_NATIVE_API_VERSION) < 0) {
        return -1;
    }
    if (PyModule_AddStringConstant(module, "runtime", "cp312-subinterpreter-native-probe") < 0) {
        return -1;
    }
    return 0;
}

static PyModuleDef_Slot ghost_slots[] = {
    {Py_mod_exec, ghost_exec},
    {Py_mod_multiple_interpreters, Py_MOD_PER_INTERPRETER_GIL_SUPPORTED},
    {0, NULL}
};

static struct PyModuleDef ghost_module = {
    PyModuleDef_HEAD_INIT,
    "ghost_native",
    "Ghost & Guild FL Studio native transport probe.",
    sizeof(GhostNativeState),
    ghost_methods,
    ghost_slots,
    NULL,
    NULL,
    NULL
};

PyMODINIT_FUNC
PyInit_ghost_native(void)
{
    return PyModuleDef_Init(&ghost_module);
}
