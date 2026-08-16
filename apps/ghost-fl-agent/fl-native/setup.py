from setuptools import Extension, setup

setup(
    name="ghost-native",
    version="0.0.1",
    ext_modules=[
        Extension(
            "ghost_native",
            ["ghost_native.c"],
            libraries=["Ws2_32"],
        )
    ],
)
