class Config {
    constructor(name) {
        this.name = name;
        this.version = '1.0.0';
        this.settings = new Map();
    }

    set(key, value) {
        this.settings.set(key, value);
    }

    get(key) {
        return this.settings.get(key);
    }

    process(data) {
        return data.join('');
    }
}

class DataProcessor {
    constructor(config) {
        this.config = config;
    }

    run(input) {
        return this.config.process(input.split(''));
    }
}

function createConfig(name) {
    return new Config(name);
}

function mergeConfigs(base, override) {
    const merged = new Config(base.name);
    for (const [k, v] of base.settings) {
        merged.set(k, v);
    }
    for (const [k, v] of override.settings) {
        merged.set(k, v);
    }
    return merged;
}

module.exports = { Config, DataProcessor, createConfig, mergeConfigs };
