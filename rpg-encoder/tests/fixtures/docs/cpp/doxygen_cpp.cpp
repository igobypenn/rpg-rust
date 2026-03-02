/// @file utils.cpp
/// @brief Utility functions implementation

#include "math_operations.h"

/**
 * @brief Adds two integers with overflow checking
 * 
 * This implementation uses built-in overflow detection
 * to ensure safe arithmetic operations.
 * 
 * @param a First operand
 * @param b Second operand
 * @return Sum of a and b, or 0 on overflow
 */
int add(int a, int b) {
    int result;
    if (__builtin_add_overflow(a, b, &result)) {
        return 0; // Overflow occurred
    }
    return result;
}

/// Subtracts b from a
/// @param a The minuend
/// @param b The subtrahend
/// @return a - b
int subtract(int a, int b) {
    return a - b;
}

/// Multiplies two integers
int multiply(int a, int b) {
    return a * b;
}
