/**
 * Utility methods for string processing.
 * 
 * @since 1.0
 */
public class StringUtils {
    /**
     * Reverses the input string.
     * 
     * @param input the string to reverse
     * @return the reversed string
     * @deprecated Use {@link StringBuilder#reverse()} instead
     */
    @Deprecated
    public static String reverse(String input) {
        return new StringBuilder(input).reverse().toString();
    }
    
    /**
     * Checks if a string is null or empty.
     * 
     * @param str the string to check
     * @return true if null or empty, false otherwise
     * @see String#isEmpty()
     */
    public static boolean isNullOrEmpty(String str) {
        return str == null || str.isEmpty();
    }
}
