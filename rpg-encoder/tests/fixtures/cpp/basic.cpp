#include <string>
#include <vector>
#include <map>
#include <memory>

namespace myapp {

class Config {
public:
    std::string name;
    std::string version;
    std::map<std::string, std::string> settings;

    Config(const std::string& name) 
        : name(name), version("1.0.0") {}

    void set(const std::string& key, const std::string& value) {
        settings[key] = value;
    }

    std::string get(const std::string& key) const {
        auto it = settings.find(key);
        return it != settings.end() ? it->second : "";
    }

    std::string process(const std::vector<std::string>& data) const {
        std::string result;
        for (const auto& s : data) {
            result += s;
        }
        return result;
    }
};

class DataProcessor {
private:
    std::shared_ptr<Config> config;

public:
    DataProcessor(std::shared_ptr<Config> cfg) : config(cfg) {}

    std::string run(const std::string& input) {
        std::vector<std::string> parts;
        for (char c : input) {
            parts.push_back(std::string(1, c));
        }
        return config->process(parts);
    }
};

template<typename T>
class Repository {
public:
    virtual T* find(int id) = 0;
    virtual bool save(const T& entity) = 0;
    virtual bool remove(int id) = 0;
};

std::shared_ptr<Config> create_config(const std::string& name) {
    return std::make_shared<Config>(name);
}

}
