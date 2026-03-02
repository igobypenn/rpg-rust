using System;

namespace Example
{
    /// <summary>
    /// Calculator class providing basic arithmetic operations.
    /// </summary>
    /// <remarks>
    /// <para>
    /// This class is thread-safe and can be used in concurrent applications.
    /// All methods are static and do not maintain state.
    /// </para>
    /// </remarks>
    /// <example>
    /// <code>
    /// var result = Calculator.Add(5, 3);
    /// Console.WriteLine(result); // Output: 8
    /// </code>
    /// </example>
    public static class Calculator
    {
        /// <summary>
        /// Adds two integers together.
        /// </summary>
        /// <param name="a">The first operand.</param>
        /// <param name="b">The second operand.</param>
        /// <returns>The sum of <paramref name="a"/> and <paramref name="b"/>.</returns>
        /// <exception cref="OverflowException">
        /// Thrown when the result exceeds <see cref="Int32.MaxValue"/>.
        /// </exception>
        public static int Add(int a, int b) => checked(a + b);

        /// <summary>
        /// Multiplies two integers.
        /// </summary>
        /// <param name="a">First factor.</param>
        /// <param name="b">Second factor.</param>
        /// <returns>The product.</returns>
        public static int Multiply(int a, int b) => a * b;
    }
}
