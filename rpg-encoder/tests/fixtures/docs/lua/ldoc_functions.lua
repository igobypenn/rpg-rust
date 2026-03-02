--- Calculates the sum of two numbers.
-- This function performs addition with type coercion.
-- @param a number The first number
-- @param b number The second number
-- @return number The sum of a and b
-- @usage
--   local result = calculate_sum(1, 2)
--   print(result) -- Output: 3
function calculate_sum(a, b)
    return a + b
end

--- User class for managing user data.
-- @classmod User
--
-- This class provides methods for creating and managing
-- user instances with name and email properties.
--
-- @usage
--   local user = User:new("John", "john@example.com")
--   user:greet()
local User = {}
User.__index = User

--- Create a new User instance.
-- @param name string The user's name
-- @param email string The user's email
-- @return User A new User instance
function User:new(name, email)
    local self = setmetatable({}, User)
    self.name = name
    self.email = email
    return self
end

--- Greet the user.
-- @return string A greeting message
function User:greet()
    return "Hello, " .. self.name .. "!"
end

return User
