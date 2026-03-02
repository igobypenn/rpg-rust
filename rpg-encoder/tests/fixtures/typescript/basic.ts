interface IConfig {
    name: string;
    version: string;
    get(key: string): string | undefined;
    set(key: string, value: string): void;
}

class Config implements IConfig {
    public name: string;
    public version: string = '1.0.0';
    private settings: Map<string, string> = new Map();

    constructor(name: string) {
        this.name = name;
    }

    set(key: string, value: string): void {
        this.settings.set(key, value);
    }

    get(key: string): string | undefined {
        return this.settings.get(key);
    }

    process(data: string[]): string {
        return data.join('');
    }
}

interface Processor<T> {
    process(input: T): T;
}

class DataProcessor implements Processor<string> {
    private config: Config;

    constructor(config: Config) {
        this.config = config;
    }

    process(input: string): string {
        return this.config.process(input.split(''));
    }
}

type ConfigFactory = (name: string) => Config;

const createConfig: ConfigFactory = (name) => new Config(name);

function mergeConfigs(base: Config, override: Config): Config {
    const merged = new Config(base.name);
    return merged;
}

export { Config, DataProcessor, createConfig, mergeConfigs };
export type { IConfig, Processor, ConfigFactory };
