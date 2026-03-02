require 'json'

module MyApp
  class Config
    attr_accessor :name, :version, :settings

    def initialize(name)
      @name = name
      @version = '1.0.0'
      @settings = {}
    end

    def set(key, value)
      @settings[key] = value
    end

    def get(key)
      @settings[key]
    end

    def process(data)
      data.join('')
    end
  end

  class Processor
    def initialize(config)
      @config = config
    end

    def run(input)
      @config.process(input.split(''))
    end
  end

  module Utils
    def self.merge_configs(base, override)
      merged = base.dup
      override.settings.each { |k, v| merged.set(k, v) }
      merged
    end
  end
end

def create_config(name)
  MyApp::Config.new(name)
end
