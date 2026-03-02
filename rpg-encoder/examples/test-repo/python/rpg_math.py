"""
Python bindings for the RPG Math library.

This module provides Python wrappers around the Rust FFI functions.
"""

import ctypes
from ctypes import c_int, c_void_p, POINTER
from pathlib import Path
from typing import Optional


class Config(ctypes.Structure):
    """Configuration for complex operations."""

    _fields_ = [
        ("precision", c_int),
        ("rounding_mode", c_int),
    ]


class StringResult(ctypes.Structure):
    """Result from string operations."""

    _fields_ = [
        ("data", POINTER(ctypes.c_char)),
        ("len", ctypes.c_size_t),
        ("capacity", ctypes.c_size_t),
    ]


def _load_library() -> ctypes.CDLL:
    """Load the RPG math library."""
    # Try common locations
    lib_names = [
        "librpg_math.so",
        "librpg_math.dylib",
        "rpg_math.dll",
    ]

    for lib_name in lib_names:
        lib_path = (
            Path(__file__).parent.parent / "rust" / "target" / "release" / lib_name
        )
        if lib_path.exists():
            return ctypes.CDLL(str(lib_path))

    # Fallback to system library
    try:
        return ctypes.CDLL("rpg_math")
    except OSError:
        raise RuntimeError("Could not load RPG math library")


_lib = None


def _get_lib():
    """Get or load the library."""
    global _lib
    if _lib is None:
        _lib = _load_library()
    return _lib


def add(a: int, b: int) -> int:
    """Add two integers using the Rust library.

    Args:
        a: First integer
        b: Second integer

    Returns:
        The sum of a and b
    """
    lib = _get_lib()
    lib.rpg_add.argtypes = [c_int, c_int]
    lib.rpg_add.restype = c_int
    return lib.rpg_add(a, b)


def multiply(a: int, b: int) -> int:
    """Multiply two integers using the Rust library.

    Args:
        a: First integer
        b: Second integer

    Returns:
        The product of a and b
    """
    lib = _get_lib()
    lib.rpg_multiply.argtypes = [c_int, c_int]
    lib.rpg_multiply.restype = c_int
    return lib.rpg_multiply(a, b)


def subtract(a: int, b: int) -> int:
    """Subtract b from a using the Rust library.

    Args:
        a: First integer
        b: Second integer

    Returns:
        a minus b
    """
    lib = _get_lib()
    lib.rpg_subtract.argtypes = [c_int, c_int]
    lib.rpg_subtract.restype = c_int
    return lib.rpg_subtract(a, b)


def process(value: int, precision: int = 1, rounding_mode: int = 0) -> int:
    """Process a value with configuration.

    Args:
        value: The input value
        precision: Precision multiplier (default 1)
        rounding_mode: 0 for integer, 1 for float (default 0)

    Returns:
        The processed result
    """
    lib = _get_lib()
    lib.rpg_process.argtypes = [c_int, POINTER(Config)]
    lib.rpg_process.restype = c_int

    config = Config(precision=precision, rounding_mode=rounding_mode)
    return lib.rpg_process(value, ctypes.byref(config))


def greet(name: str = "World") -> str:
    """Create a greeting string.

    Args:
        name: The name to greet (default "World")

    Returns:
        A greeting string
    """
    lib = _get_lib()
    lib.rpg_greet.argtypes = [ctypes.c_char_p]
    lib.rpg_greet.restype = StringResult
    lib.rpg_free_string.argtypes = [StringResult]
    lib.rpg_free_string.restype = None

    name_bytes = name.encode("utf-8")
    result = lib.rpg_greet(name_bytes)

    try:
        greeting = ctypes.string_at(result.data, result.len).decode("utf-8")
        return greeting
    finally:
        lib.rpg_free_string(result)


class MathClient:
    """High-level client for RPG Math operations."""

    def __init__(self, lib_path: Optional[str] = None):
        """Initialize the client.

        Args:
            lib_path: Optional path to the library
        """
        self._lib_path = lib_path

    def add(self, a: int, b: int) -> int:
        """Add two numbers."""
        return add(a, b)

    def multiply(self, a: int, b: int) -> int:
        """Multiply two numbers."""
        return multiply(a, b)

    def process(self, value: int, precision: int = 1) -> int:
        """Process a value."""
        return process(value, precision)


# CFFI alternative implementation
try:
    import cffi

    CFFI_DEFINITIONS = """
        typedef struct {
            int precision;
            int rounding_mode;
        } Config;
        
        int rpg_add(int a, int b);
        int rpg_multiply(int a, int b);
        int rpg_subtract(int a, int b);
        int rpg_process(int value, const Config* config);
    """

    def create_cffi_client(lib_path: str):
        """Create a CFFI-based client."""
        ffi = cffi.FFI()
        ffi.cdef(CFFI_DEFINITIONS)
        lib = ffi.dlopen(lib_path)
        return ffi, lib

except ImportError:
    pass
