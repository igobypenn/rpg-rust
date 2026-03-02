package com.example

/** Calculator utility class.
  *
  * This class provides basic arithmetic operations.
  * All methods are pure functions with no side effects.
  *
  * @example {{{
  * val calc = new Calculator()
  * val result = calc.add(5, 3)
  * println(result) // Output: 8
  * }}}
  *
  * @constructor Creates a new Calculator instance
  * @see Math
  */
class Calculator {
  
  /** Adds two integers together.
    *
    * @param a the first operand
    * @param b the second operand
    * @return the sum of a and b
    */
  def add(a: Int, b: Int): Int = a + b
  
  /** Multiplies two integers.
    *
    * @param a the first factor
    * @param b the second factor
    * @return the product of a and b
    */
  def multiply(a: Int, b: Int): Int = a * b
}

/** User case class with immutable fields.
  *
  * @param name the user's name
  * @param email the user's email address
  */
case class User(name: String, email: String) {
  
  /** Creates a greeting for the user.
    *
    * @return a personalized greeting message
    */
  def greet(): String = s"Hello, $name!"
}

/** Enumeration of user statuses.
  *
  * Use this to track the current state of a user account.
  */
enum UserStatus {
  /** Account is active and can log in */
  case Active
  
  /** Account is temporarily suspended */
  case Suspended
  
  /** Account is permanently banned */
  case Banned
}
