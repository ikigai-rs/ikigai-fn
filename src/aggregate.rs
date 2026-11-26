//! Reporting types for aggregate counts about people.
//!
//! Apply the floor to each published count. This is a display rule, not a
//! guarantee of anonymity: overlapping counts or a published ratio and its
//! denominator can reveal counts by arithmetic.

use std::fmt;

use serde::{Serialize, Serializer};

/// The smallest count of people reported as an exact number.
pub const COUNT_FLOOR: usize = 5;

fn below_floor() -> String {
    format!("fewer than {COUNT_FLOOR}")
}

fn display<T: fmt::Display>(
    value: Option<T>,
    suffix: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match value {
        Some(value) => write!(f, "{value}{suffix}"),
        None => f.write_str(&below_floor()),
    }
}

fn serialize<T: Serialize, S: Serializer>(
    value: Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => value.serialize(serializer),
        None => serializer.serialize_str(&below_floor()),
    }
}

/// A reportable count. A suppressed count does not retain its input.
///
/// Display uses plain text. JSON uses a number for exact counts and the same
/// plain-text message for suppressed counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count(Option<usize>);

impl Count {
    /// Apply the shared floor to a count of people.
    ///
    /// The same value can serve a plain-text count line and a JSON field.
    ///
    /// ```
    /// use ikigai_fn::aggregate::Count;
    ///
    /// let joined = Count::new(4);
    /// assert_eq!(format!("joined {joined}"), "joined fewer than 5");
    /// assert_eq!(serde_json::json!({ "joined": joined }), serde_json::json!({
    ///     "joined": "fewer than 5"
    /// }));
    /// ```
    pub fn new(count: usize) -> Self {
        Self((count >= COUNT_FLOOR).then_some(count))
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self.0, "", f)
    }
}

impl Serialize for Count {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(self.0, serializer)
    }
}

/// The fraction of a group that belongs to a subset, or a suppression message
/// if the group is below the floor. A suppressed ratio retains neither input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio(Option<f64>);

impl Ratio {
    /// Return `None` for an invalid subset (`part > group`). A valid empty
    /// group produces a suppressed ratio instead of dividing by zero.
    pub fn new(part: usize, group: usize) -> Option<Self> {
        if part > group {
            return None;
        }
        Some(Self(
            Count::new(group).0.map(|group| part as f64 / group as f64),
        ))
    }

    /// Convert the ratio to a percentage without undoing suppression.
    pub fn percent(self) -> Percent {
        Percent(self.0.map(|ratio| ratio * 100.0))
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self.0, "", f)
    }
}

impl Serialize for Ratio {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(self.0, serializer)
    }
}

/// A percentage derived from a reportable ratio. Exact values display with a
/// percent sign, but serialize as JSON numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percent(Option<f64>);

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self.0, "%", f)
    }
}

impl Serialize for Percent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(self.0, serializer)
    }
}
