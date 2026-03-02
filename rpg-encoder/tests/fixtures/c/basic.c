#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* name;
    char* version;
    int count;
} Config;

Config* config_new(const char* name) {
    Config* cfg = malloc(sizeof(Config));
    cfg->name = strdup(name);
    cfg->version = strdup("1.0.0");
    cfg->count = 0;
    return cfg;
}

void config_free(Config* cfg) {
    free(cfg->name);
    free(cfg->version);
    free(cfg);
}

void config_set_count(Config* cfg, int count) {
    cfg->count = count;
}

int config_get_count(const Config* cfg) {
    return cfg->count;
}

char* process_data(const char* input, size_t len) {
    char* output = malloc(len + 1);
    memcpy(output, input, len);
    output[len] = '\0';
    return output;
}

int add_numbers(int a, int b) {
    return a + b;
}

typedef int (*callback_fn)(int);

int call_callback(callback_fn fn, int value) {
    return fn(value);
}
