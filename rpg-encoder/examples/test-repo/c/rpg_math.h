/**
 * @file rpg_math.h
 * @brief C interface for RPG Math library
 * 
 * This header provides C bindings for the RPG Math library,
 * which is implemented in Rust.
 */

#ifndef RPG_MATH_H
#define RPG_MATH_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Configuration for complex operations.
 */
typedef struct {
    int precision;
    int rounding_mode;
} RpgConfig;

/**
 * Result from string operations.
 */
typedef struct {
    char* data;
    size_t len;
    size_t capacity;
} RpgStringResult;

/**
 * Adds two integers.
 * 
 * @param a First integer
 * @param b Second integer
 * @return The sum of a and b
 */
int rpg_add(int a, int b);

/**
 * Multiplies two integers.
 * 
 * @param a First integer
 * @param b Second integer
 * @return The product of a and b
 */
int rpg_multiply(int a, int b);

/**
 * Subtracts b from a.
 * 
 * @param a First integer
 * @param b Second integer
 * @return a minus b
 */
int rpg_subtract(int a, int b);

/**
 * Processes a value with configuration.
 * 
 * @param value The input value
 * @param config Pointer to configuration struct
 * @return The processed result
 */
int rpg_process(int value, const RpgConfig* config);

/**
 * Creates a greeting string.
 * 
 * @param name The name to greet (can be NULL for "World")
 * @return A string result that must be freed with rpg_free_string
 */
RpgStringResult rpg_greet(const char* name);

/**
 * Frees a string result.
 * 
 * @param result The string result to free
 */
void rpg_free_string(RpgStringResult result);

/**
 * Initializes the library.
 * 
 * @return 1 on success, 0 on failure
 */
int rpg_init(void);

/**
 * Shuts down the library.
 */
void rpg_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* RPG_MATH_H */
