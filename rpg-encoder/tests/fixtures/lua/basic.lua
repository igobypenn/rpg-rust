local Config = {}
Config.__index = Config

function Config:new(name)
    local self = setmetatable({}, Config)
    self.name = name
    self.version = "1.0.0"
    self.settings = {}
    return self
end

function Config:set(key, value)
    self.settings[key] = value
end

function Config:get(key)
    return self.settings[key]
end

function Config:process(data)
    return table.concat(data, "")
end

local DataProcessor = {}
DataProcessor.__index = DataProcessor

function DataProcessor:new(config)
    local self = setmetatable({}, DataProcessor)
    self.config = config
    return self
end

function DataProcessor:run(input)
    local parts = {}
    for char in input:gmatch(".") do
        table.insert(parts, char)
    end
    return self.config:process(parts)
end

local function createConfig(name)
    return Config:new(name)
end

local function mergeConfigs(base, override)
    local merged = Config:new(base.name)
    for k, v in pairs(base.settings) do
        merged:set(k, v)
    end
    for k, v in pairs(override.settings) do
        merged:set(k, v)
    end
    return merged
end

return {
    Config = Config,
    DataProcessor = DataProcessor,
    createConfig = createConfig,
    mergeConfigs = mergeConfigs
}
