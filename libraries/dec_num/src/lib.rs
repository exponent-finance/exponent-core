use exponent_time_curve::num::Num;
use precise_number::Number;
use rust_decimal::{
    prelude::{FromPrimitive, One, ToPrimitive},
    Decimal, MathematicalOps,
};
use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, Default)]
pub struct DNum {
    pub value: Decimal,
}

// The maximum size of a Decimal is 96 bits
const MAX_U96: u128 = (1 << 96) - 1;

impl DNum {
    pub fn deserialize(bs: &[u8]) -> Self {
        let ar: [u8; 16] = match bs.try_into() {
            Ok(ar) => ar,
            Err(_) => {
                panic!("Unable to deserialize bytes")
            }
        };

        let value = Decimal::deserialize(ar);
        Self { value }
    }

    pub fn from_precise_number(pn: &Number) -> Self {
        let mut num = pn.to_pn().value.as_u128();
        let den = precise_number::Number::DENOM;

        // the numerator might be too big (greater than 96 bits)
        // so scale it down, sacrificing decimal precision
        let mut n = 0;
        while num > MAX_U96 {
            n += 1;
            num /= 10; // Scale down by one decimal place
        }

        // scale down the denominator appropriately
        let den = den / 10u128.pow(n as u32);
        let num = Decimal::from_u128(num).unwrap();
        let den = Decimal::from_u128(den).unwrap();

        let value = num / den;

        Self { value }
    }

    pub fn to_precise_number(&self) -> precise_number::Number {
        let d = self.value;
        if d.is_zero() {
            return precise_number::Number::ZERO;
        }

        assert!(d.is_sign_positive());
        let int_part = d.trunc();
        let frac_part = d - int_part;
        let scale = Decimal::from_u128(precise_number::Number::DENOM)
            .expect("unable to conert denom to Decimal");

        let scaled_frac_part = (frac_part * scale).trunc();

        let int_part = precise_number::Number::from(int_part.to_u128().unwrap());
        let frac_part = precise_number::Number::from_ratio(
            scaled_frac_part.to_u128().unwrap(),
            precise_number::Number::DENOM,
        );

        int_part + frac_part
    }

    pub fn abs(self) -> Self {
        if self.value.is_sign_negative() {
            Self { value: -self.value }
        } else {
            self
        }
    }

    pub fn is_sign_negative(&self) -> bool {
        self.value.is_sign_negative()
    }

    pub fn is_sign_positive(&self) -> bool {
        self.value.is_sign_positive()
    }
}

impl From<i64> for DNum {
    fn from(value: i64) -> Self {
        Self {
            value: Decimal::from_i64(value).unwrap(),
        }
    }
}

impl From<f64> for DNum {
    fn from(value: f64) -> Self {
        Self {
            value: Decimal::from_f64(value).unwrap(),
        }
    }
}

impl Into<i64> for DNum {
    fn into(self) -> i64 {
        self.value.to_i64().unwrap()
    }
}

impl PartialEq for DNum {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

impl Neg for DNum {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self { value: -self.value }
    }
}

impl PartialOrd for DNum {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Div for DNum {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value / rhs.value,
        }
    }
}

impl Mul for DNum {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value * rhs.value,
        }
    }
}

impl Sub for DNum {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value - rhs.value,
        }
    }
}

impl Add for DNum {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value + rhs.value,
        }
    }
}

impl std::fmt::Display for DNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

fn checked_ln(d: &Decimal) -> Option<Decimal> {
    if d.is_sign_negative() || d.is_zero() {
        return None;
    }
    if d.is_one() {
        return Some(Decimal::ZERO);
    }

    // Approximate using Taylor Series
    let mut x = *d;
    let mut count = 0;
    while x >= Decimal::ONE {
        x *= Decimal::E_INVERSE;
        count += 1;
    }
    while x <= Decimal::E_INVERSE {
        x *= Decimal::E;
        count -= 1;
    }
    x -= Decimal::ONE;
    if x.is_zero() {
        return Some(Decimal::new(count, 0));
    }
    let mut result = Decimal::ZERO;
    let mut iteration = 0;
    let mut y = Decimal::ONE;
    let mut last = Decimal::ONE;
    while last != result && iteration < 50 {
        iteration += 1;
        last = result;
        y *= -x;
        result += y / Decimal::new(iteration, 0);
    }
    Some(Decimal::new(count, 0) - result)
}

fn ln(d: &Decimal) -> Decimal {
    match checked_ln(d) {
        Some(result) => result,
        None => {
            if d.is_sign_negative() {
                panic!("Unable to calculate ln for negative numbers")
            } else if d.is_zero() {
                panic!("Unable to calculate ln for zero")
            } else {
                panic!("Calculation of ln failed for unknown reasons")
            }
        }
    }
}

impl Num for DNum {
    fn ln(&self) -> Self {
        Self {
            value: ln(&self.value),
        }
    }

    fn exp(&self) -> Self {
        Self {
            value: self.value.exp(),
        }
    }

    fn one() -> Self {
        Self {
            value: Decimal::one(),
        }
    }

    fn zero() -> Self {
        Self {
            value: Decimal::ZERO,
        }
    }

    fn from_ratio(numerator: u64, denominator: u64) -> Self {
        let value = Decimal::from_u64(numerator).unwrap() / Decimal::from_u64(denominator).unwrap();

        Self { value }
    }

    fn min(self, other: Self) -> Self {
        let value = self.value.min(other.value);
        Self { value }
    }

    fn from_u64(value: u64) -> Self {
        Self {
            value: Decimal::from_u64(value).unwrap(),
        }
    }

    fn to_u64(&self) -> u64 {
        self.value.to_u64().unwrap()
    }

    fn to_i64(&self) -> i64 {
        self.value.to_i64().unwrap()
    }

    fn max() -> Self {
        Self {
            value: Decimal::MAX,
        }
    }
}
