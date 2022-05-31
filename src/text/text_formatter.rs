use crate::types::timestamp::Precision;
use crate::{Decimal, Integer, IonResult, IonType, Timestamp};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Timelike};
use std::convert::TryInto;
use std::fmt::Write;

pub struct IonValueFormatter<'a, W: Write> {
    output: &'a mut W,
}

impl<'a, W: Write> IonValueFormatter<'a, W> {
    pub fn format_null(&mut self, ion_type: IonType) -> IonResult<()> {
        use IonType::*;
        let null_text = match ion_type {
            Null => "null",
            Boolean => "null.bool",
            Integer => "null.int",
            Float => "null.float",
            Decimal => "null.decimal",
            Timestamp => "null.timestamp",
            Symbol => "null.symbol",
            String => "null.string",
            Blob => "null.blob",
            Clob => "null.clob",
            List => "null.list",
            SExpression => "null.sexp",
            Struct => "null.struct",
        };
        write!(self.output, "{}", null_text)?;
        Ok(())
    }

    pub fn format_bool(&mut self, value: bool) -> IonResult<()> {
        let bool_text = match value {
            true => "true",
            false => "false",
        };
        write!(self.output, "{}", bool_text)?;
        Ok(())
    }

    pub fn format_integer(&mut self, value: &Integer) -> IonResult<()> {
        match value {
            Integer::I64(i) => write!(self.output, "{}", i)?,
            Integer::BigInt(i) => write!(self.output, "{}", i)?,
        }
        Ok(())
    }

    pub fn format_float(&mut self, value: f64) -> IonResult<()> {
        if value.is_nan() {
            write!(self.output, "nan")?;
            return Ok(());
        }

        if value.is_infinite() {
            if value.is_sign_positive() {
                write!(self.output, "+inf")?;
            } else {
                write!(self.output, "-inf")?;
            }
            return Ok(());
        }

        // The {:e} formatter provided by the Display trait writes floats using scientific
        // notation. It works for all floating point values except -0.0 (it drops the sign).
        // See: https://github.com/rust-lang/rust/issues/20596
        if value == 0.0f64 && value.is_sign_negative() {
            write!(self.output, "-0e0")?;
            return Ok(());
        }

        write!(self.output, "{:e}", value)?;
        Ok(())
    }

    pub fn format_decimal(&mut self, value: &Decimal) -> IonResult<()> {
        write!(self.output, "{}", value)?;
        Ok(())
    }

    pub fn format_timestamp(&mut self, value: &Timestamp) -> IonResult<()> {
        let (offset_minutes, datetime) = if let Some(minutes) = value.offset {
            // Create a datetime with the appropriate offset that we can use for formatting.
            let datetime: DateTime<FixedOffset> = value.clone().try_into()?;
            // Convert the offset to minutes --v
            (Some(minutes.local_minus_utc() / 60), datetime)
        } else {
            // Our timestamp has an unknown offset. Per the spec, this means it makes no
            // assertions about *where* it was recorded, but its fields are still in UTC.
            // Create a UTC datetime that we can use for formatting.
            let datetime: NaiveDateTime = value.clone().try_into()?;
            let datetime: DateTime<FixedOffset> = FixedOffset::east(0).from_utc_datetime(&datetime);
            (None, datetime)
        };

        write!(self.output, "{:0>4}", datetime.year())?;
        //                     ^-- 0-padded, right aligned, 4-digit year
        if value.precision == Precision::Year {
            write!(self.output, "T")?;
            return Ok(());
        }

        write!(self.output, "-{:0>2}", datetime.month())?;
        //                   ^-- delimiting hyphen and 0-padded, right aligned, 2-digit month
        if value.precision == Precision::Month {
            write!(self.output, "T")?;
            return Ok(());
        }

        write!(self.output, "-{:0>2}", datetime.day())?;
        //                   ^-- delimiting hyphen and 0-padded, right aligned, 2-digit day
        if value.precision == Precision::Day {
            write!(self.output, "T")?;
            return Ok(());
        }

        write!(
            self.output,
            "T{:0>2}:{:0>2}",
            datetime.hour(),
            datetime.minute()
        )?;
        //                   ^-- delimiting T, formatted hour, delimiting colon, formatted minute
        if value.precision == Precision::HourAndMinute {
            self.format_offset(offset_minutes)?;
            return Ok(());
        }

        write!(self.output, ":{:0>2}", datetime.second())?;
        //                   ^-- delimiting colon, formatted second
        value.format_fractional_seconds(&mut *self.output)?;

        self.format_offset(offset_minutes)?;
        Ok(())
    }

    fn format_offset(&mut self, offset_minutes: Option<i32>) -> IonResult<()> {
        if offset_minutes.is_none() {
            write!(self.output, "-00:00")?;
            return Ok(());
        }
        let offset_minutes = offset_minutes.unwrap();

        const MINUTES_PER_HOUR: i32 = 60;
        // Split the offset into a sign and magnitude for formatting
        let sign = if offset_minutes >= 0 { "+" } else { "-" };
        let offset_minutes = offset_minutes.abs();
        let hours = offset_minutes / MINUTES_PER_HOUR;
        let minutes = offset_minutes % MINUTES_PER_HOUR;
        write!(self.output, "{}{:0>2}:{:0>2}", sign, hours, minutes)?;
        Ok(())
    }
}

mod formatter_test {
    use crate::text::text_formatter::IonValueFormatter;
    use crate::{Integer, IonResult, IonType, Timestamp};
    use num_bigint::BigInt;

    fn formatter<F>(mut f: F, expected: &str)
    where
        F: for<'a> FnMut(&mut IonValueFormatter<'a, String>) -> IonResult<()>,
    {
        let mut actual = String::new();
        let mut ivf = IonValueFormatter {
            output: &mut actual,
        };

        let _ = f(&mut ivf);

        assert_eq!(actual, expected)
    }

    #[test]
    fn test_format_null() -> IonResult<()> {
        formatter(|ivf| ivf.format_null(IonType::Symbol), "null.symbol");
        formatter(|ivf| ivf.format_null(IonType::Null), "null");
        Ok(())
    }

    #[test]
    fn test_format_bool() -> IonResult<()> {
        formatter(|ivf| ivf.format_bool(true), "true");
        formatter(|ivf| ivf.format_bool(false), "false");
        Ok(())
    }

    #[test]
    fn test_format_i64() -> IonResult<()> {
        formatter(|ivf| ivf.format_integer(&Integer::I64(4)), "4");
        formatter(|ivf| ivf.format_integer(&Integer::I64(-4)), "-4");
        Ok(())
    }

    #[test]
    fn test_format_big_int() -> IonResult<()> {
        formatter(
            |ivf| ivf.format_integer(&Integer::BigInt(BigInt::from(4))),
            "4",
        );
        formatter(
            |ivf| ivf.format_integer(&Integer::BigInt(BigInt::from(-4))),
            "-4",
        );
        Ok(())
    }

    #[test]
    fn test_format_float() -> IonResult<()> {
        formatter(|ivf| ivf.format_float(400f64), "4e2");
        formatter(|ivf| ivf.format_float(-400f64), "-4e2");
        Ok(())
    }

    #[test]
    fn test_format_timestamp() -> IonResult<()> {
        let timestamp = Timestamp::with_year(2000)
            .with_month(8)
            .build()
            .expect("building timestamp failed");
        formatter(|ivf| ivf.format_timestamp(&timestamp), "2000-08T");
        Ok(())
    }
}
