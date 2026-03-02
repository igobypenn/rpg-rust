import Foundation

struct Config {
    var name: String
    var version: String = "1.0.0"
    private var settings: [String: String] = [:]

    mutating func set(_ key: String, _ value: String) {
        settings[key] = value
    }

    func get(_ key: String) -> String? {
        return settings[key]
    }

    func process(_ data: [String]) -> String {
        return data.joined()
    }
}

class DataProcessor {
    private var config: Config

    init(config: Config) {
        self.config = config
    }

    func run(_ input: String) -> String {
        let parts = input.map { String($0) }
        return config.process(parts)
    }
}

protocol Repository {
    associatedtype Entity
    func find(id: Int) -> Entity?
    func save(_ entity: Entity) -> Bool
    func delete(id: Int) -> Bool
}

func createConfig(name: String) -> Config {
    return Config(name: name)
}
