package main

// Calculator performs basic arithmetic operations.
//
// It supports addition, subtraction, multiplication, and division.
// Example:
//
//	calc := Calculator{}
//	result := calc.Add(5, 3) // result = 8
type Calculator struct{}

// Add returns the sum of two integers.
func (c Calculator) Add(a, b int) int {
	return a + b
}

// Multiply returns the product of two integers.
func (c Calculator) Multiply(a, b int) int {
	return a * b
}
