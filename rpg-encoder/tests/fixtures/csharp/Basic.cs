using System;
using System.Collections.Generic;
using System.Linq;

namespace MyApp
{
    public class Config
    {
        public string Name { get; set; }
        public string Version { get; set; } = "1.0.0";
        private Dictionary<string, string> _settings = new();

        public void Set(string key, string value)
        {
            _settings[key] = value;
        }

        public string? Get(string key)
        {
            return _settings.TryGetValue(key, out var value) ? value : null;
        }

        public string Process(List<string> data)
        {
            return string.Join("", data);
        }
    }

    public class DataProcessor
    {
        private readonly Config _config;

        public DataProcessor(Config config)
        {
            _config = config;
        }

        public string Run(string input)
        {
            var parts = input.Select(c => c.ToString()).ToList();
            return _config.Process(parts);
        }
    }

    public interface IRepository<T>
    {
        T? Find(long id);
        bool Save(T entity);
        bool Delete(long id);
    }

    public record User(long Id, string Name, string Email);

    public static class Factory
    {
        public static Config CreateConfig(string name)
        {
            return new Config { Name = name };
        }
    }
}
