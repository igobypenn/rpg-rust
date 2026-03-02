/// A point in 2D space.
///
/// This struct represents a coordinate with x and y values.
///
/// # Examples
///
/// ```
/// let point = Point { x: 1.0, y: 2.0 };
/// ```
pub struct Point {
    /// The x coordinate
    pub x: f64,
    /// The y coordinate
    pub y: f64,
}

/// An enum with documented variants.
pub enum Status {
    /// The operation was successful
    Success,
    /// The operation failed with an error
    Failure,
    /// The operation is still pending
    Pending,
}
