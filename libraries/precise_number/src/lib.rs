use anchor_lang::{AnchorDeserialize, AnchorSerialize};
use bytemuck::{Pod, Zeroable};
use spl_math::{
    precise_number::{self, *},
    uint::U256,
};
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

/// High precision number, stored as 4 u64 words in little endian
#[derive(
    Default, Clone, Debug, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Zeroable, Pod,
)]
#[repr(C)]
pub struct Number([u64; 4]);

impl core::fmt::Display for Number {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let n = U256(self.0);
        write!(f, "{}", n)
    }
}

impl From<u64> for Number {
    fn from(value: u64) -> Self {
        let pn = PreciseNumber::new(value.into()).unwrap();
        Number(pn.value.0)
    }
}

impl From<u128> for Number {
    fn from(value: u128) -> Self {
        let pn = PreciseNumber::new(value).unwrap();
        Number(pn.value.0)
    }
}

/// Wrapper around PreciseNumber (a U256) that has 1e12 precision
impl Number {
    // The byte size of Number
    pub const SIZEOF: usize = 32;

    pub const ZERO: Self = Self(U256::zero().0);

    // It just so happens that 1 is 10e12, which is smaller than a u64
    pub const ONE: Self = Self([precise_number::ONE as u64, 0, 0, 0]);

    pub const DENOM: u128 = precise_number::ONE;

    pub fn from_bytes_le(slice: &[u8]) -> Self {
        Self(U256::from_little_endian(slice).0)
    }

    pub fn from_natural_u64(value: u64) -> Self {
        value.into()
    }

    pub fn from_ratio(num: u128, den: u128) -> Self {
        let num = PreciseNumber::new(num).unwrap();
        let den = PreciseNumber::new(den).unwrap();

        PreciseNumber::checked_div(&num, &den)
            .unwrap_or(PreciseNumber::new(0).unwrap())
            .into()
    }

    /// Convert BPS into Number
    pub fn from_bps(bps: u16) -> Self {
        Self::from_natural_u64(bps as u64) / Self::from_natural_u64(10_000)
    }

    pub fn checked_add(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_add(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn checked_sub(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_sub(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    /// Wrap-around subtraction (mod 2^256).
    pub fn wrapping_sub(&self, x: &Self) -> Self {
        let (value, _) = U256(self.0).overflowing_sub(U256(x.0));
        Self(value.0)
    }

    pub fn checked_mul(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_mul(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn checked_div(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_div(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn to_pn(&self) -> PreciseNumber {
        PreciseNumber {
            value: U256(self.0),
        }
    }

    pub fn min(numbers: &[Self]) -> Self {
        *numbers.iter().min().unwrap()
    }

    pub fn floor_u64(&self) -> u64 {
        self.to_pn()
            .floor()
            .unwrap()
            .to_imprecise()
            .unwrap()
            .try_into()
            .unwrap()
    }

    pub fn ceil(&self) -> u128 {
        self.to_pn().ceiling().unwrap().to_imprecise().unwrap()
    }

    pub fn ceil_u64(&self) -> u64 {
        self.ceil().try_into().unwrap()
    }

    pub fn floor_u128(&self) -> u128 {
        self.to_pn().floor().unwrap().to_imprecise().unwrap()
    }

    pub fn to_f64(&self) -> Option<f64> {
        // scale up by 1e12 to avoid precision loss
        let n = self
            .checked_mul(&Number::from(Self::DENOM))
            .unwrap()
            .to_pn()
            .to_imprecise()
            .unwrap();
        // denominator is always 1e12
        let d = precise_number::ONE;
        u128_to_f64_checked(n, d)
    }




    /// Panics if the value overflows u128 (upper two words non-zero).
    #[inline(always)]
    pub fn raw_u128(&self) -> u128 {
        assert!(
            self.0[2] == 0 && self.0[3] == 0,
            "Number too large for u128"
        );
        (self.0[0] as u128) | ((self.0[1] as u128) << 64)
    }

    /// Construct a Number from a raw scaled u128 value (already multiplied by DENOM).
    /// This is the inverse of `raw_u128()`.
    #[inline(always)]
    pub fn from_raw_u128(raw: u128) -> Self {
        Self([raw as u64, (raw >> 64) as u64, 0, 0])
    }


    #[inline(always)]
    pub fn fast_floor_u128(&self) -> u128 {
        self.raw_u128() / Self::DENOM
    }


    /// Equivalent to `Number::from(value)` but uses only u128 arithmetic.
    #[inline(always)]
    pub fn fast_from_u128(value: u128) -> Self {
        let scaled = value
            .checked_mul(Self::DENOM)
            .expect("Number::fast_from_u128 overflow");
        Self::from_raw_u128(scaled)
    }


    #[inline(always)]
    pub fn fast_add_u128(&self, other: &Self) -> Self {
        let a = self.raw_u128();
        let b = other.raw_u128();
        Self::from_raw_u128(a.checked_add(b).expect("Number::fast_add overflow"))
    }


    #[inline(always)]
    pub fn fast_sub_u128(&self, other: &Self) -> Self {
        let a = self.raw_u128();
        let b = other.raw_u128();
        Self::from_raw_u128(a.checked_sub(b).expect("Number::fast_sub underflow"))
    }


    /// Uses raw u128 extraction + f64 division.
    #[inline(always)]
    pub fn fast_to_f64(&self) -> f64 {
        let raw = self.raw_u128();
        (raw as f64) / (Self::DENOM as f64)
    }


    /// Computes floor(self * other) using split-multiply to avoid u128 overflow.
    ///
    /// Algorithm: decompose each raw value as (hi * DENOM + lo), then:
    ///   a*b / DENOM² = a_hi*b_hi + cross/DENOM + lo_prod/DENOM²
    /// where cross = a_hi*b_lo + a_lo*b_hi and lo_prod = a_lo*b_lo.
    ///
    /// The fractional part from `cross % DENOM` can combine with `lo_prod` to add
    /// an extra carry of 1, so we must account for:
    ///   carry = floor(((cross % DENOM) * DENOM + lo_prod) / DENOM²).
    #[inline(always)]
    pub fn fast_mul_floor_u64(&self, other: &Self) -> u64 {
        let a = self.raw_u128();
        let b = other.raw_u128();
        let d = Self::DENOM;
        let d2 = d
            .checked_mul(d)
            .expect("Number::DENOM^2 overflow in fast_mul_floor_u64");

        let a_hi = a / d;
        let a_lo = a % d;
        let b_hi = b / d;
        let b_lo = b % d;

        let cross = a_hi
            .checked_mul(b_lo)
            .and_then(|v| v.checked_add(a_lo.checked_mul(b_hi).unwrap()))
            .expect("fast_mul_floor_u64 cross term overflow");
        let cross_div = cross / d;
        let cross_rem = cross % d;

        let lo_prod = a_lo
            .checked_mul(b_lo)
            .expect("fast_mul_floor_u64 low term overflow");
        let carry = cross_rem
            .checked_mul(d)
            .and_then(|v| v.checked_add(lo_prod))
            .expect("fast_mul_floor_u64 carry term overflow")
            / d2;

        let result = a_hi
            .checked_mul(b_hi)
            .unwrap()
            .checked_add(cross_div)
            .and_then(|v| v.checked_add(carry))
            .unwrap();

        result
            .try_into()
            .expect("fast_mul_floor_u64 result does not fit in u64")
    }

    /// Fast multiply a Number by a u64 integer, returning a Number.
    /// Computes self * integer without going through PreciseNumber.
    /// Safe when self.raw * integer fits in u128.
    #[inline(always)]
    pub fn fast_mul_u64(&self, integer: u64) -> Self {
        let raw = self
            .raw_u128()
            .checked_mul(integer as u128)
            .expect("Number::fast_mul_u64 overflow");
        Self::from_raw_u128(raw)
    }

    /// Fast divide a Number by a u64 integer, returning a Number.
    /// Computes self / integer without going through PreciseNumber.
    #[inline(always)]
    pub fn fast_div_u64(&self, integer: u64) -> Self {
        let raw = self.raw_u128() / (integer as u128);
        Self::from_raw_u128(raw)
    }

    /// Fast multiply Number by a ratio (num/den), returning a Number.
    /// Uses u128 arithmetic: (self_raw * num) / den.
    /// Safe when self_raw * num fits in u128.
    #[inline(always)]
    pub fn fast_mul_ratio(&self, num: u128, den: u128) -> Self {
        let raw = self.raw_u128();
        let result = raw.checked_mul(num).expect("fast_mul_ratio overflow") / den;
        Self::from_raw_u128(result)
    }
}

/// Convert a u128 ratio to f64, checking for overflow
fn u128_to_f64_checked(numerator: u128, denominator: u128) -> Option<f64> {
    // Constants for maximum values f64 can represent exactly
    const MAX_EXACT_U64: u128 = (1u128 << 53) - 1;

    if denominator == 0 {
        return None; // Division by zero is invalid
    }

    // Enforce exact representability for both operands before conversion.
    if numerator > MAX_EXACT_U64 || denominator > MAX_EXACT_U64 {
        return None;
    }

    Some((numerator as f64) / (denominator as f64))
}

impl From<PreciseNumber> for Number {
    fn from(pn: PreciseNumber) -> Self {
        Self(pn.value.0)
    }
}

impl Add<Number> for Number {
    type Output = Self;

    fn add(self, rhs: Number) -> Self::Output {
        Self(self.to_pn().checked_add(&rhs.to_pn()).unwrap().value.0)
    }
}

impl Mul<Number> for Number {
    type Output = Self;

    fn mul(self, x: Number) -> Self::Output {
        Self(self.checked_mul(&x).unwrap().0)
    }
}

impl Div<Number> for Number {
    type Output = Self;

    fn div(self, x: Number) -> Self {
        Self(self.checked_div(&x).unwrap().0)
    }
}

impl AddAssign<Number> for Number {
    fn add_assign(&mut self, rhs: Number) {
        self.0 = self.checked_add(&rhs).unwrap().0;
    }
}

impl SubAssign<Number> for Number {
    fn sub_assign(&mut self, rhs: Number) {
        self.0 = self.checked_sub(&rhs).unwrap().0;
    }
}

impl Sub<Number> for Number {
    type Output = Self;

    fn sub(self, x: Number) -> Self {
        self.checked_sub(&x).unwrap()
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        U256(self.0).cmp(&U256(other.0))
    }
}
