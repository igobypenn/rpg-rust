package main

/*
#include <stdlib.h>
#include <string.h>

int add_numbers(int a, int b) {
    return a + b;
}

int process_data(const void* data, size_t len) {
    if (data == NULL) return -1;
    return 0;
}
*/
import "C"
import "unsafe"

func AddViaCgo(a, b int) int {
	return int(C.add_numbers(C.int(a), C.int(b)))
}

func ProcessViaCgo(data []byte) int {
	return int(C.process_data(unsafe.Pointer(&data[0]), C.size_t(len(data))))
}

//export GoExportedFunction
func GoExportedFunction(x int) int {
	return x * 2
}

//export AnotherExported
func AnotherExported(s string) int {
	return len(s)
}
