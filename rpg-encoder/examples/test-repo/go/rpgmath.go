// Package rpgmath provides Go bindings for the RPG Math library.
//
// This package wraps the Rust FFI functions for use in Go applications.
package rpgmath

/*
#cgo CFLAGS: -I${SRCDIR}/../c
#cgo LDFLAGS: -L${SRCDIR}/../rust/target/release -lrpg_math -ldl

#include "rpg_math.h"
#include <stdlib.h>
*/
import "C"

import (
	"errors"
	"unsafe"
)

// Config represents the configuration for complex operations.
type Config struct {
	Precision    int32
	RoundingMode int32
}

// toC converts Go Config to C RpgConfig.
func (c *Config) toC() C.RpgConfig {
	return C.RpgConfig{
		precision:     C.int(c.Precision),
		rounding_mode: C.int(c.RoundingMode),
	}
}

// Add adds two integers using the Rust library.
func Add(a, b int32) int32 {
	return int32(C.rpg_add(C.int(a), C.int(b)))
}

// Multiply multiplies two integers using the Rust library.
func Multiply(a, b int32) int32 {
	return int32(C.rpg_multiply(C.int(a), C.int(b)))
}

// Subtract subtracts b from a using the Rust library.
func Subtract(a, b int32) int32 {
	return int32(C.rpg_subtract(C.int(a), C.int(b)))
}

// Process processes a value with the given configuration.
func Process(value int32, config *Config) int32 {
	if config == nil {
		config = &Config{Precision: 1, RoundingMode: 0}
	}
	cConfig := config.toC()
	return int32(C.rpg_process(C.int(value), &cConfig))
}

// Greet creates a greeting string for the given name.
func Greet(name string) string {
	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cName))

	result := C.rpg_greet(cName)
	defer C.rpg_free_string(result)

	return C.GoStringN(result.data, C.int(result.len))
}

// Init initializes the library.
func Init() error {
	if C.rpg_init() == 0 {
		return errors.New("failed to initialize rpg_math library")
	}
	return nil
}

// Shutdown shuts down the library.
func Shutdown() {
	C.rpg_shutdown()
}

// BatchProcess processes multiple values at once.
func BatchProcess(values []int32, precision int32) ([]int32, error) {
	if len(values) == 0 {
		return nil, nil
	}

	results := make([]int32, len(values))
	config := Config{Precision: precision, RoundingMode: 0}

	for i, v := range values {
		results[i] = Process(v, &config)
	}

	return results, nil
}

// SafeProcess validates input before processing.
func SafeProcess(value int32, precision int32) (int32, error) {
	if value < 0 || value > 10000 {
		return 0, errors.New("value out of valid range [0, 10000]")
	}
	return Process(value, &Config{Precision: precision}), nil
}

// MathClient provides a high-level interface to the RPG Math library.
type MathClient struct {
	defaultPrecision int32
	initialized      bool
}

// NewMathClient creates a new MathClient.
func NewMathClient() (*MathClient, error) {
	if err := Init(); err != nil {
		return nil, err
	}
	return &MathClient{
		defaultPrecision: 1,
		initialized:      true,
	}, nil
}

// Close releases resources.
func (c *MathClient) Close() {
	if c.initialized {
		Shutdown()
		c.initialized = false
	}
}

// Add adds two numbers.
func (c *MathClient) Add(a, b int32) int32 {
	return Add(a, b)
}

// Multiply multiplies two numbers.
func (c *MathClient) Multiply(a, b int32) int32 {
	return Multiply(a, b)
}

// Process processes a value with the default precision.
func (c *MathClient) Process(value int32) int32 {
	return Process(value, &Config{Precision: c.defaultPrecision})
}

// SetDefaultPrecision sets the default precision for operations.
func (c *MathClient) SetDefaultPrecision(p int32) {
	c.defaultPrecision = p
}
