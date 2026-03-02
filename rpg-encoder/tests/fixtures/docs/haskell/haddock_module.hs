-- | Represents a user in the system.
-- This data type contains basic user information
-- including name and email address.
--
-- Example:
--
-- >>> let user = User "John" "john@example.com"
-- >>> userName user
-- "John"
data User = User
    { userName :: String  -- ^ The user's full name
    , userEmail :: String -- ^ The user's email address
    }
  deriving (Show, Eq)

-- | Create a new User with a name.
-- This constructor sets the email to an empty string.
--
-- >>> newUser "Jane"
-- User {userName = "Jane", userEmail = ""}
newUser :: String -> User
newUser name = User name ""

-- | Get a greeting message for the user.
-- Returns a personalized greeting including the user's name.
--
-- >>> greet (User "John" "john@example.com")
-- "Hello, John!"
greet :: User -> String
greet (User name _) = "Hello, " ++ name ++ "!"
