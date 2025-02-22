// Copyright Amazon.com, Inc. or its affiliates.

use std::io::Write;

use arrayvec::ArrayVec;
use bigdecimal::Zero;

use crate::ion_eq::IonEq;
use crate::{
    binary::{
        int::DecodedInt, raw_binary_writer::MAX_INLINE_LENGTH, var_int::VarInt, var_uint::VarUInt,
    },
    result::IonResult,
    types::{
        coefficient::{Coefficient, Sign},
        decimal::Decimal,
        magnitude::Magnitude,
    },
};

const DECIMAL_BUFFER_SIZE: usize = 16;
const DECIMAL_POSITIVE_ZERO: Decimal = Decimal {
    coefficient: Coefficient {
        sign: Sign::Positive,
        magnitude: Magnitude::U64(0),
    },
    exponent: 0,
};

/// Provides support to write [`Decimal`] into [Ion binary].
///
/// [Ion binary]: https://amzn.github.io/ion-docs/docs/binary.html#5-decimal
pub trait DecimalBinaryEncoder {
    /// Encodes the content of a [`Decimal`] as per the Ion binary encoding.
    /// Returns the length of the encoded bytes.
    ///
    /// This does not encode the type descriptor nor the associated length.
    /// Prefer [`DecimalBinaryEncoder::encode_decimal_value`] for that.
    fn encode_decimal(&mut self, decimal: &Decimal) -> IonResult<usize>;

    /// Encodes a [`Decimal`] as an Ion value with the type descriptor and
    /// length. Returns the length of the encoded bytes.
    fn encode_decimal_value(&mut self, decimal: &Decimal) -> IonResult<usize>;
}

impl<W> DecimalBinaryEncoder for W
where
    W: Write,
{
    fn encode_decimal(&mut self, decimal: &Decimal) -> IonResult<usize> {
        // 0d0 has no representation, as per the spec.
        if decimal.ion_eq(&DECIMAL_POSITIVE_ZERO) {
            return Ok(0);
        }

        let mut bytes_written: usize = 0;

        bytes_written += VarInt::write_i64(self, decimal.exponent)?;

        if decimal.coefficient.is_negative_zero() {
            bytes_written += DecodedInt::write_negative_zero(self)?;
            return Ok(bytes_written);
        }

        // If the coefficient is small enough to safely fit in an i64, use that to avoid
        // allocating.
        if let Some(small_coefficient) = decimal.coefficient.as_i64() {
            // From the spec: "The subfield should not be present (that is, it
            // has zero length) when the coefficient’s value is (positive)
            // zero."
            if !small_coefficient.is_zero() {
                bytes_written += DecodedInt::write_i64(self, small_coefficient)?;
            }
        } else {
            // Otherwise, allocate a Vec<u8> with the necessary representation.
            let mut coefficient_bytes = match decimal.coefficient.magnitude() {
                Magnitude::U64(unsigned) => unsigned.to_be_bytes().into(),
                Magnitude::BigUInt(big) => big.to_bytes_be(),
            };

            let first_byte: &mut u8 = &mut coefficient_bytes[0];
            let first_bit_is_zero: bool = *first_byte & 0b1000_0000 == 0;
            if let Sign::Negative = decimal.coefficient.sign() {
                // If the first bit is unset, it's now the sign bit. Set it to 1.
                if first_bit_is_zero {
                    *first_byte |= 0b1000_0000;
                } else {
                    // Otherwise, we need to write out an extra leading byte with a sign bit set
                    self.write_all(&[0b1000_0000])?;
                    bytes_written += 1;
                }
            } else {
                // If the first bit is unset, it's now the sign bit.
                if first_bit_is_zero {
                    // Do nothing; zero is the correct sign bit for a non-negative coefficient.
                } else {
                    // Otherwise, we need to write out an extra leading byte with an unset sign bit
                    self.write_all(&[0b0000_0000])?;
                    bytes_written += 1;
                }
            }
            self.write_all(coefficient_bytes.as_slice())?;
            bytes_written += coefficient_bytes.len();
        }

        Ok(bytes_written)
    }

    fn encode_decimal_value(&mut self, decimal: &Decimal) -> IonResult<usize> {
        let mut bytes_written: usize = 0;
        // First encode the decimal. We need to know the encoded length before
        // we can compute and write out the type descriptor.
        let mut encoded: ArrayVec<u8, DECIMAL_BUFFER_SIZE> = ArrayVec::new();
        encoded.encode_decimal(decimal)?;

        let type_descriptor: u8;
        if encoded.len() <= MAX_INLINE_LENGTH {
            type_descriptor = 0x50 | encoded.len() as u8;
            self.write_all(&[type_descriptor])?;
            bytes_written += 1;
        } else {
            type_descriptor = 0x5E;
            self.write_all(&[type_descriptor])?;
            bytes_written += 1;
            bytes_written += VarUInt::write_u64(self, encoded.len() as u64)?;
        }

        // Now we can write out the encoded decimal!
        self.write_all(&encoded[..])?;
        bytes_written += encoded.len();

        Ok(bytes_written)
    }
}

#[cfg(test)]
mod binary_decimal_tests {
    use super::*;
    use rstest::*;

    /// This test ensures that we implement [PartialEq] and [IonEq] correctly for the special
    /// decimal value 0d0.
    #[test]
    fn decimal_0d0_is_a_special_zero_value() {
        assert_eq!(DECIMAL_POSITIVE_ZERO, Decimal::new(0, 0));
        assert!(DECIMAL_POSITIVE_ZERO.ion_eq(&Decimal::new(0, 0)));

        assert_eq!(DECIMAL_POSITIVE_ZERO, Decimal::new(0, 10));
        assert!(!DECIMAL_POSITIVE_ZERO.ion_eq(&Decimal::new(0, 10)));

        assert_eq!(DECIMAL_POSITIVE_ZERO, Decimal::new(0, 100));
        assert!(!DECIMAL_POSITIVE_ZERO.ion_eq(&Decimal::new(0, 100)));
    }

    #[rstest]
    #[case::exactly_zero(Decimal::new(0, 0), 1)]
    #[case::zero_with_nonzero_exp(Decimal::new(0, 10), 2)]
    #[case::meaning_of_life(Decimal::new(42, 0), 3)]
    fn bytes_written(#[case] input: Decimal, #[case] expected: usize) -> IonResult<()> {
        let mut buf = vec![];
        let written = buf.encode_decimal_value(&input)?;
        assert_eq!(buf.len(), expected);
        assert_eq!(written, expected);
        Ok(())
    }
}
