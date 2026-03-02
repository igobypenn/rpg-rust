#include <cstdint>
#include <cstddef>

extern "C" {
    int32_t add_numbers(int32_t a, int32_t b) {
        return a + b;
    }

    int32_t process_data(const void* data, size_t len) {
        if (data == nullptr) return -1;
        return 0;
    }

    void* allocate_buffer(size_t size) {
        return new char[size];
    }

    void free_buffer(void* ptr) {
        delete[] static_cast<char*>(ptr);
    }
}

extern "C" int external_callback(int (*fn)(int), int value) {
    return fn(value);
}
