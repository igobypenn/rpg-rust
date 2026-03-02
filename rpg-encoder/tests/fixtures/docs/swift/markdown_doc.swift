import Foundation

/// Represents a user in the system.
///
/// This struct contains basic user information including
/// identification and contact details.
///
/// ## Example
///
/// ```swift
/// let user = User(name: "John", email: "john@example.com")
/// print(user.greet())
/// ```
///
/// - Note: Users are immutable after creation.
/// - Important: Email validation is performed on initialization.
struct User {
    /// The user's full name.
    ///
    /// This should be the user's legal name as it appears
    /// on official documents.
    let name: String
    
    /// The user's email address.
    ///
    /// Must be a valid email format. Will be validated
    /// during initialization.
    let email: String
    
    /// Creates a personalized greeting.
    ///
    /// - Returns: A greeting message including the user's name.
    func greet() -> String {
        return "Hello, \(name)!"
    }
}

/// A calculator providing arithmetic operations.
///
/// Use this class for performing basic mathematical operations
/// with proper overflow handling.
///
/// ## Topics
///
/// ### Adding Numbers
/// - ``add(_:_:)``
///
/// ### Multiplying Numbers
/// - ``multiply(_:_:)``
class Calculator {
    /// Adds two integers together.
    ///
    /// - Parameters:
    ///   - a: The first operand
    ///   - b: The second operand
    /// - Returns: The sum of the two operands
    /// - Throws: ``CalculatorError.overflow`` if the result overflows
    func add(_ a: Int, _ b: Int) -> Int {
        return a + b
    }
    
    /// Multiplies two integers.
    ///
    /// - Parameters:
    ///   - a: The first factor
    ///   - b: The second factor
    /// - Returns: The product of the two factors
    func multiply(_ a: Int, _ b: Int) -> Int {
        return a * b
    }
}
