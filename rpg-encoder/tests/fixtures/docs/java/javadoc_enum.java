/**
 * User status enumeration.
 * 
 * Defines the possible states of a user account.
 * 
 * @since 1.0
 */
public enum UserStatus {
    /**
     * User account is active and can log in.
     */
    ACTIVE,
    
    /**
     * User account is temporarily suspended.
     */
    SUSPENDED,
    
    /**
     * User account is permanently banned.
     */
    BANNED,
    
    /**
     * User account is pending email verification.
     */
    PENDING
}
