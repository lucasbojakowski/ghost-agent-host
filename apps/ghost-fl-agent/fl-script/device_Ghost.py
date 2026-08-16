# name=Ghost Bridge
# supportedDevices=Ghost Midi

import ghost_native

print(ghost_native.runtime)
print(ghost_native.API_VERSION)
print(ghost_native.ping())
print(ghost_native.pid())
print(ghost_native.socket_probe())
print(ghost_native.connect_probe("127.0.0.1", 48766))