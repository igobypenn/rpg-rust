/**
 * User representation.
 * 
 * This record represents a user in the system with basic information.
 * 
 * @param name the user's full name
 * @param email the user's email address
 * @param age the user's age in years
 */
public record User(String name, String email, int age) {
    /**
     * Creates a new User with validated fields.
     * 
     * @param name the user's name (must not be null or blank)
     * @param email the user's email (must be a valid email format)
     * @param age the user's age (must be non-negative)
     * @throws IllegalArgumentException if any validation fails
     */
    public User {
        if (name == null || name.isBlank()) {
            throw new IllegalArgumentException("Name must not be blank");
        }
        if (!email.contains("@")) {
            throw new IllegalArgumentException("Invalid email format");
        }
        if (age < 0) {
            throw new IllegalArgumentException("Age must be non-negative");
        }
    }
}
