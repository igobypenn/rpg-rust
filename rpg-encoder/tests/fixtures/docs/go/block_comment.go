package main

/*
User represents a user in the system.

This struct contains basic user information including
identification and contact details.

Example:
  user := User{
      Name:  "John Doe",
      Email: "john@example.com",
      Age:   30,
  }
*/
type User struct {
	Name  string // The user's full name
	Email string // The user's email address
	Age   int    // The user's age
}

/*
NewUser creates a new User instance with the given name.
It initializes the user with default email and age values.
*/
func NewUser(name string) *User {
	return &User{
		Name:  name,
		Email: "unknown@example.com",
		Age:   0,
	}
}
