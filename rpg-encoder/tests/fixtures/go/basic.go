package main

import (
	"fmt"
	"strings"
)

type Config struct {
	Name    string
	Version string
	Settings map[string]string
}

func NewConfig(name string) *Config {
	return &Config{
		Name:     name,
		Version:  "1.0.0",
		Settings: make(map[string]string),
	}
}

func (c *Config) Set(key, value string) {
	c.Settings[key] = value
}

func (c *Config) Get(key string) (string, bool) {
	v, ok := c.Settings[key]
	return v, ok
}

func (c *Config) Process(data []string) string {
	return strings.Join(data, "")
}

func CreateConfig(name string) *Config {
	return NewConfig(name)
}

func MergeConfigs(base, override *Config) *Config {
	merged := NewConfig(base.Name)
	for k, v := range base.Settings {
		merged.Settings[k] = v
	}
	for k, v := range override.Settings {
		merged.Settings[k] = v
	}
	return merged
}

func main() {
	cfg := CreateConfig("test")
	cfg.Set("key", "value")
	fmt.Println(cfg.Process([]string{"a", "b", "c"}))
}
