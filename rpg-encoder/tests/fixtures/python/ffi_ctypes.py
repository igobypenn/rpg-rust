import ctypes
from ctypes import c_int, c_void_p, CDLL

mylib = ctypes.CDLL("libmylib.so")

mylib.add_numbers.argtypes = [c_int, c_int]
mylib.add_numbers.restype = c_int


def add_via_ctypes(a: int, b: int) -> int:
    return mylib.add_numbers(a, b)


process_data = mylib.process_data
process_data.argtypes = [c_void_p, ctypes.c_size_t]
process_data.restype = c_int
