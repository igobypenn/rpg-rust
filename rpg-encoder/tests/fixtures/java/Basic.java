package com.example.app;

import java.util.HashMap;
import java.util.Map;
import java.util.List;
import java.util.ArrayList;

public class Config {
    private String name;
    private String version;
    private Map<String, String> settings;

    public Config(String name) {
        this.name = name;
        this.version = "1.0.0";
        this.settings = new HashMap<>();
    }

    public void set(String key, String value) {
        settings.put(key, value);
    }

    public String get(String key) {
        return settings.get(key);
    }

    public String process(List<String> data) {
        return String.join("", data);
    }

    public String getName() { return name; }
    public String getVersion() { return version; }
}

class DataProcessor {
    private Config config;

    public DataProcessor(Config config) {
        this.config = config;
    }

    public String run(String input) {
        List<String> parts = new ArrayList<>();
        for (char c : input.toCharArray()) {
            parts.add(String.valueOf(c));
        }
        return config.process(parts);
    }
}

interface Repository<T> {
    T find(long id);
    boolean save(T entity);
    boolean delete(long id);
}

public class Basic {
    public static Config createConfig(String name) {
        return new Config(name);
    }

    public static void main(String[] args) {
        Config cfg = createConfig("test");
        System.out.println(cfg.getName());
    }
}
