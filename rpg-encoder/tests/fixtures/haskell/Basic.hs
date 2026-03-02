module MyApp.Config where

import Data.Map (Map)
import qualified Data.Map as Map

data Config = Config
    { configName :: String
    , configVersion :: String
    , configSettings :: Map String String
    }

newConfig :: String -> Config
newConfig name = Config
    { configName = name
    , configVersion = "1.0.0"
    , configSettings = Map.empty
    }

setSetting :: String -> String -> Config -> Config
setSetting key value cfg = cfg { configSettings = Map.insert key value (configSettings cfg) }

getSetting :: String -> Config -> Maybe String
getSetting key cfg = Map.lookup key (configSettings cfg)

process :: [String] -> String
process = concat

class Repository a where
    find :: Int -> Maybe a
    save :: a -> Bool
    delete :: Int -> Bool

data User = User
    { userId :: Int
    , userName :: String
    , userEmail :: String
    } deriving (Show, Eq)

createConfig :: String -> Config
createConfig = newConfig
