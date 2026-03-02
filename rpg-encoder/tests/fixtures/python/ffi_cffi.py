from cffi import FFI

ffi = FFI()
ffi.cdef("""
    int add_numbers(int a, int b);
    int process_data(const void* data, size_t len);
    void* allocate_buffer(size_t size);
""")

lib = ffi.dlopen("libmylib.so")


def add_via_cffi(a: int, b: int) -> int:
    return lib.add_numbers(a, b)


def process_via_cffi(data: bytes) -> int:
    return lib.process_data(ffi.new("char[]", data), len(data))
