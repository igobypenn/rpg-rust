/**
 * Calculator utility class.
 * 
 * <p>This class provides basic arithmetic operations with proper error handling.
 * It is thread-safe and can be used in concurrent applications.</p>
 * 
 * <h2>Usage Example:</h2>
 * <pre>{@code
 * Calculator calc = new Calculator();
 * int result = calc.add(5, 3);
 * System.out.println(result); // Output: 8
 * }</pre>
 * 
 * @author John Doe
 * @version 1.0
 * @since 2024-01-01
 * @see Math
 */
public class Calculator {
    /**
     * Adds two integers together.
     * 
     * <p>This method performs integer addition with overflow checking.
     * If overflow occurs, an exception is thrown.</p>
     * 
     * @param a the first operand
     * @param b the second operand
     * @return the sum of {@code a} and {@code b}
     * @throws ArithmeticException if the result overflows an int
     */
    public int add(int a, int b) {
        return Math.addExact(a, b);
    }
    
    /**
     * Divides two integers.
     * 
     * @param dividend the number to be divided
     * @param divisor the number to divide by
     * @return the result of the division
     * @throws ArithmeticException if {@code divisor} is zero
     */
    public int divide(int dividend, int divisor) {
        return dividend / divisor;
    }
    
    /**
     * Multiplies two integers.
     * 
     * @param a the first factor
     * @param b the second factor
     * @return the product of {@code a} and {@code b}
     */
    public int multiply(int a, int b) {
        return a * b;
    }
}
