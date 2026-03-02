def markdown_example(value):
    """Process a value with **markdown** formatting.

    # Heading 1
    ## Heading 2

    This function supports:
    - **Bold text**
    - *Italic text*
    - `code snippets`
    - [Links](https://example.com)

    ```python
    # Code block example
    result = markdown_example(42)
    print(result)
    ```

    > This is a blockquote
    > with multiple lines

    | Column 1 | Column 2 |
    |----------|----------|
    | Value 1  | Value 2  |

    Args:
        value: Input value

    Returns:
        Processed value
    """
    return value * 2
