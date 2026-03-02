# This is a module-level comment
# It describes what this module does

# The following function processes data
def process_data(data):
    """Process the input data.

    Args:
        data: Input data to process

    Returns:
        Processed data
    """
    # Validate the input first
    if not data:
        return None

    # Apply transformations
    result = data.strip().lower()

    return result
