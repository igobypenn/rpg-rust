class Calculator:
    """A simple calculator class.

    This class provides basic arithmetic operations.

    Attributes:
        history: List of previous calculations

    Examples:
        >>> calc = Calculator()
        >>> calc.add(1, 2)
        3
    """

    def __init__(self):
        """Initialize the calculator with empty history."""
        self.history = []

    def add(self, a, b):
        """Add two numbers and store in history.

        Args:
            a: First number
            b: Second number

        Returns:
            The sum of a and b
        """
        result = a + b
        self.history.append(("add", a, b, result))
        return result

    def subtract(self, a, b):
        """Subtract b from a and store in history."""
        result = a - b
        self.history.append(("subtract", a, b, result))
        return result
