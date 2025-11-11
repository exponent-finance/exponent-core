use std::ops::{Add, Div, Mul, Neg, Sub};

/// Encapsulated trait for the number types used in the library.
/// Will probably be rust_decimal::Decimal in the future as the implementation for this
pub trait Num:
    Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + Copy
    + Clone
    + PartialEq
    + PartialOrd
    + Default
    + std::fmt::Debug
    + std::fmt::Display
{
    /// Returns the natural logarithm of the number.
    fn ln(&self) -> Self;

    fn abs(&self) -> Self {
        if *self < Self::zero() {
            -*self
        } else {
            *self
        }
    }

    /// Returns the exponential of the number.
    fn exp(&self) -> Self;

    fn one() -> Self;

    fn zero() -> Self;

    fn from_ratio(numerator: u64, denominator: u64) -> Self;

    fn min(self, other: Self) -> Self {
        if self < other {
            self
        } else {
            other
        }
    }

    fn from_u64(value: u64) -> Self;

    fn to_u64(&self) -> u64;

    fn to_i64(&self) -> i64;

    fn from_i64(value: i64) -> Self {
        if value < 0 {
            -Self::from_u64(value.abs() as u64)
        } else {
            Self::from_u64(value as u64)
        }
    }

    fn max() -> Self;
}

// Usage of f64 to optimize compute unit usage
impl Num for f64 {
    fn ln(&self) -> Self {
        <f64>::ln(*self)
    }

    fn exp(&self) -> Self {
        <f64>::exp(*self)
    }

    fn one() -> Self {
        1.0
    }

    fn zero() -> Self {
        0.0
    }

    fn from_ratio(numerator: u64, denominator: u64) -> Self {
        numerator as f64 / denominator as f64
    }

    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    fn from_u64(value: u64) -> Self {
        value as f64
    }

    fn to_u64(&self) -> u64 {
        *self as u64
    }

    fn to_i64(&self) -> i64 {
        *self as i64
    }

    fn max() -> Self {
        f64::MAX
    }
}