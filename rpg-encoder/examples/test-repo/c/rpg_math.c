/**
 * @file rpg_math.c
 * @brief C wrapper implementation for RPG Math library
 * 
 * This file provides a C wrapper around the Rust FFI functions,
 * adding additional utility functions.
 */

#include "rpg_math.h"
#include <stdlib.h>
#include <string.h>

/* External Rust functions */
extern int rpg_add(int a, int b);
extern int rpg_multiply(int a, int b);
extern int rpg_subtract(int a, int b);
extern int rpg_process(int value, const RpgConfig* config);
extern RpgStringResult rpg_greet(const char* name);
extern void rpg_free_string(RpgStringResult result);

/* Internal state */
static int g_initialized = 0;

/**
 * Validates input values.
 */
static int validate_input(int value) {
    return value >= 0 && value <= 10000;
}

/**
 * Creates a default configuration.
 */
RpgConfig rpg_default_config(void) {
    RpgConfig config;
    config.precision = 1;
    config.rounding_mode = 0;
    return config;
}

/**
 * Safe process with input validation.
 */
int rpg_safe_process(int value, int precision) {
    if (!validate_input(value)) {
        return -1;
    }
    
    RpgConfig config = rpg_default_config();
    config.precision = precision;
    
    return rpg_process(value, &config);
}

/**
 * Initializes the library.
 */
int rpg_init(void) {
    if (g_initialized) {
        return 1;
    }
    
    g_initialized = 1;
    return 1;
}

/**
 * Shuts down the library.
 */
void rpg_shutdown(void) {
    g_initialized = 0;
}

/**
 * Batch processing of values.
 */
int rpg_batch_process(const int* values, int count, int precision, int* results) {
    if (!g_initialized || values == NULL || results == NULL) {
        return -1;
    }
    
    RpgConfig config = rpg_default_config();
    config.precision = precision;
    
    for (int i = 0; i < count; i++) {
        if (validate_input(values[i])) {
            results[i] = rpg_process(values[i], &config);
        } else {
            results[i] = -1;
        }
    }
    
    return 0;
}

/**
 * Creates a formatted greeting.
 */
char* rpg_format_greeting(const char* name, const char* title) {
    if (name == NULL) {
        name = "World";
    }
    if (title == NULL) {
        title = "";
    }
    
    size_t name_len = strlen(name);
    size_t title_len = strlen(title);
    size_t total_len = name_len + title_len + 20;
    
    char* result = (char*)malloc(total_len);
    if (result == NULL) {
        return NULL;
    }
    
    if (title_len > 0) {
        snprintf(result, total_len, "Hello, %s %s!", title, name);
    } else {
        snprintf(result, total_len, "Hello, %s!", name);
    }
    
    return result;
}

/**
 * Frees a formatted greeting.
 */
void rpg_free_greeting(char* greeting) {
    if (greeting != NULL) {
        free(greeting);
    }
}
