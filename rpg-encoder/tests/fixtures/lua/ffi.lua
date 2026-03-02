local ffi = require("ffi")

ffi.cdef[[
    int add_numbers(int a, int b);
    int process_data(const void* data, size_t len);
    void* malloc(size_t size);
    void free(void* ptr);
    size_t strlen(const char* s);
]]

local mylib = ffi.load("mylib")

local function addViaFfi(a, b)
    return mylib.add_numbers(a, b)
end

local function processViaFfi(data)
    local cdata = ffi.new("char[?]", #data + 1)
    ffi.copy(cdata, data)
    return mylib.process_data(cdata, #data)
end

local function useStdLib(s)
    return ffi.C.strlen(s)
end

local function allocateBuffer(size)
    local ptr = ffi.C.malloc(size)
    return ptr
end

local int_ptr = ffi.new("int[10]")
local struct_ptr = ffi.new("struct { int x; int y; }")

return {
    addViaFfi = addViaFfi,
    processViaFfi = processViaFfi,
    useStdLib = useStdLib
}
