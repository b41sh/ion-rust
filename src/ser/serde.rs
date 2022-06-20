use serde::ser::{
    self, Error as SerdeError, Serialize, SerializeMap, SerializeSeq, SerializeStruct,
    SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};

use std::collections::HashMap;
use crate::Decimal;
use crate::Timestamp;
use crate::types::integer::Integer;
use crate::value::owned::{OwnedElement, OwnedSequence, OwnedStruct, OwnedValue};
use crate::value::{Builder, Element, Sequence};
use crate::IonType;
use bigdecimal::ToPrimitive;
use num_bigint::ToBigInt;

use super::Error;

impl Serialize for OwnedElement {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match &self.value {
            OwnedValue::Null(_) => serializer.serialize_unit(),
            OwnedValue::Integer(v) => match v {
                Integer::I64(v) => serializer.serialize_i64(*v),
                Integer::BigInt(v) => serializer.serialize_u64(v.to_u64().unwrap()),
            },
            OwnedValue::Float(v) => serializer.serialize_f64(*v),
            OwnedValue::Decimal(v) => v.serialize(serializer),
            OwnedValue::Timestamp(v) => v.serialize(serializer),
            OwnedValue::String(v) => serializer.serialize_str(&v),
            OwnedValue::Symbol(_) => serializer.serialize_unit(),
            OwnedValue::Boolean(v) => serializer.serialize_bool(*v),
            OwnedValue::Blob(v) => serializer.serialize_bytes(v.as_slice()),
            OwnedValue::Clob(v) => serializer.serialize_bytes(v.as_slice()),
            OwnedValue::SExpression(_) => serializer.serialize_unit(),
            OwnedValue::List(list_val) => {
                let mut seq = serializer.serialize_seq(Some(list_val.len()))?;
                for v in list_val.iter() {
                    seq.serialize_element(v)?;
                }
                seq.end()
            }
            OwnedValue::Struct(_) => serializer.serialize_unit(),
        }
    }
}

pub fn to_element_with_options<T: ?Sized>(
    value: &T,
    options: SerializerOptions,
) -> crate::ser::Result<OwnedElement>
where
    T: Serialize,
{
    let ser = Serializer::new_with_options(options);
    value.serialize(ser)
}

/// Serde Serializer
#[non_exhaustive]
pub struct Serializer {
    options: SerializerOptions,
}

/// Options used to configure a [`Serializer`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SerializerOptions {
    /// Whether the [`Serializer`] should present itself as human readable or not.
    /// The default value is true.
    pub human_readable: Option<bool>,
}

impl SerializerOptions {
    /// Create a builder used to construct a new [`SerializerOptions`].
    pub fn builder() -> SerializerOptionsBuilder {
        SerializerOptionsBuilder {
            options: Default::default(),
        }
    }
}

/// A builder used to construct new [`SerializerOptions`] structs.
pub struct SerializerOptionsBuilder {
    options: SerializerOptions,
}

impl SerializerOptionsBuilder {
    /// Set the value for [`SerializerOptions::is_human_readable`].
    pub fn human_readable(mut self, value: impl Into<Option<bool>>) -> Self {
        self.options.human_readable = value.into();
        self
    }

    /// Consume this builder and produce a [`SerializerOptions`].
    pub fn build(self) -> SerializerOptions {
        self.options
    }
}

impl Serializer {
    /// Construct a new `Serializer`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Serializer {
        Serializer {
            options: Default::default(),
        }
    }

    /// Construct a new `Serializer` configured with the provided [`SerializerOptions`].
    pub fn new_with_options(options: SerializerOptions) -> Self {
        Serializer { options }
    }
}

impl ser::Serializer for Serializer {
    type Ok = OwnedElement;
    type Error = Error;

    type SerializeSeq = ArraySerializer;
    type SerializeTuple = TupleSerializer;
    type SerializeTupleStruct = TupleStructSerializer;
    type SerializeTupleVariant = TupleVariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    #[inline]
    fn serialize_bool(self, value: bool) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedElement::new_bool(value))
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_u8(self, value: u8) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_u16(self, value: u16) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_u32(self, value: u32) -> crate::ser::Result<OwnedElement> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_i64(self, value: i64) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedElement::new_i64(value))
    }

    #[inline]
    fn serialize_u64(self, value: u64) -> crate::ser::Result<OwnedElement> {
        match i64::try_from(value) {
            Ok(ivalue) => Ok(OwnedElement::new_i64(ivalue)),
            Err(_) => Ok(OwnedElement::new_big_int(
                ToBigInt::to_bigint(&value).unwrap(),
            )),
        }
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> crate::ser::Result<OwnedElement> {
        self.serialize_f64(value as f64)
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedElement::new_f64(value))
    }

    #[inline]
    fn serialize_char(self, value: char) -> crate::ser::Result<OwnedElement> {
        let mut s = String::new();
        s.push(value);
        self.serialize_str(&s)
    }

    #[inline]
    fn serialize_str(self, value: &str) -> crate::ser::Result<OwnedElement> {
        //Ok(OwnedElement::new_string(value))
        Ok(value.to_string().into())
    }

    #[inline]
    fn serialize_bytes(self, value: &[u8]) -> crate::ser::Result<OwnedElement> {
        //Ok(OwnedElement::new_blob(value))
        Ok(OwnedValue::Blob(value.to_vec()).into())
    }

    #[inline]
    fn serialize_none(self) -> crate::ser::Result<OwnedElement> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_some<V: ?Sized>(self, value: &V) -> crate::ser::Result<OwnedElement>
    where
        V: Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedElement::new_null(IonType::Null))
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> crate::ser::Result<OwnedElement> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedElement::new_string(variant))
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> crate::ser::Result<OwnedElement>
    where
        T: Serialize,
    {
        match value.serialize(self) {
            Ok(element) => Ok(OwnedStruct::from_iter(vec![(name, element)].into_iter()).into()),
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> crate::ser::Result<OwnedElement>
    where
        T: Serialize,
    {
        //Some(OwnedStruct::from_iter(vec![(name, value.serialize(self)?)].into_iter()).into())
        match value.serialize(self) {
            Ok(element) => Ok(OwnedStruct::from_iter(vec![(variant, element)].into_iter()).into()),
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> crate::ser::Result<Self::SerializeSeq> {
        Ok(ArraySerializer {
            inner: Vec::with_capacity(len.unwrap_or(0)),
            options: self.options,
        })
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> crate::ser::Result<Self::SerializeTuple> {
        Ok(TupleSerializer {
            inner: Vec::with_capacity(len),
            options: self.options,
        })
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> crate::ser::Result<Self::SerializeTupleStruct> {
        Ok(TupleStructSerializer {
            inner: Vec::with_capacity(len),
            options: self.options,
        })
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> crate::ser::Result<Self::SerializeTupleVariant> {
        Ok(TupleVariantSerializer {
            inner: Vec::with_capacity(len),
            name: variant,
            options: self.options,
        })
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> crate::ser::Result<Self::SerializeMap> {
        Ok(MapSerializer {
            inner: HashMap::new(),
            next_key: None,
            options: self.options,
        })
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> crate::ser::Result<Self::SerializeStruct> {
        Ok(StructSerializer {
            inner: HashMap::new(),
            options: self.options,
        })
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> crate::ser::Result<Self::SerializeStructVariant> {
        Ok(StructVariantSerializer {
            name: variant,
            inner: HashMap::new(),
            options: self.options,
        })
    }

    fn is_human_readable(&self) -> bool {
        self.options.human_readable.unwrap_or(true)
    }
}

#[doc(hidden)]
pub struct ArraySerializer {
    inner: Vec<OwnedElement>,
    options: SerializerOptions,
}

impl SerializeSeq for ArraySerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> crate::ser::Result<()> {
        self.inner
            .push(to_element_with_options(value, self.options.clone())?);
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedSequence::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct TupleSerializer {
    inner: Vec<OwnedElement>,
    options: SerializerOptions,
}

impl SerializeTuple for TupleSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> crate::ser::Result<()> {
        self.inner
            .push(to_element_with_options(value, self.options.clone())?);
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedSequence::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct TupleStructSerializer {
    inner: Vec<OwnedElement>,
    options: SerializerOptions,
}

impl SerializeTupleStruct for TupleStructSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> crate::ser::Result<()> {
        self.inner
            .push(to_element_with_options(value, self.options.clone())?);
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedSequence::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct TupleVariantSerializer {
    inner: Vec<OwnedElement>,
    name: &'static str,
    options: SerializerOptions,
}

impl SerializeTupleVariant for TupleVariantSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> crate::ser::Result<()> {
        self.inner
            .push(to_element_with_options(value, self.options.clone())?);
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        //let mut tuple_variant = Document::new();
        //tuple_variant.insert(self.name, self.inner);
        //Ok(tuple_variant.into())
        Ok(OwnedSequence::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct MapSerializer {
    inner: HashMap<String, OwnedElement>,
    next_key: Option<String>,
    options: SerializerOptions,
}

impl SerializeMap for MapSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> crate::ser::Result<()> {
        self.next_key = match to_element_with_options(&key, self.options.clone()) {
            Ok(e) => match e.as_str() {
                Some(s) => Some(s.to_string()),
                None => return Err(Error::InvalidDocumentKey(e)),
            },
            Err(e) => return Err(e),
        };
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> crate::ser::Result<()> {
        let key = self.next_key.take().unwrap_or_default();
        self.inner
            .insert(key, to_element_with_options(&value, self.options.clone())?);
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        //Ok(OwnedStruct::from_iter::<T>(self.inner.into_iter().collect()).into())
        //Ok(OwnedStruct::from_iter(self.inner.into_iter().collect()).into())
        Ok(OwnedStruct::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct StructSerializer {
    inner: HashMap<String, OwnedElement>,
    options: SerializerOptions,
}

impl SerializeStruct for StructSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> crate::ser::Result<()> {
        self.inner.insert(
            key.to_string(),
            to_element_with_options(value, self.options.clone())?,
        );
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        //Ok(OwnedValue::Struct(self.inner).into())
        //Ok(OwnedStruct::from_iter(self.inner.into_iter().collect()).into())
        Ok(OwnedStruct::from_iter(self.inner).into())
    }
}

#[doc(hidden)]
pub struct StructVariantSerializer {
    inner: HashMap<String, OwnedElement>,
    name: &'static str,
    options: SerializerOptions,
}

impl SerializeStructVariant for StructVariantSerializer {
    type Ok = OwnedElement;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> crate::ser::Result<()> {
        self.inner.insert(
            key.to_string(),
            to_element_with_options(value, self.options.clone())?,
        );
        Ok(())
    }

    fn end(self) -> crate::ser::Result<OwnedElement> {
        Ok(OwnedStruct::from_iter(self.inner).into())
    }
}

//Decimal(Decimal),
//Timestamp(Timestamp),
//Symbol(OwnedSymbolToken),

impl Serialize for Decimal {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut state = serializer.serialize_struct("$numberDecimal", 1)?;
        //state.serialize_field("$numberDecimalBytes", serde_bytes::Bytes::new(&self.bytes))?;
        //state.serialize_field("$numberDecimalBytes", self.String())?;
        state.serialize_field("$numberDecimalBytes", self)?;
        state.end()
    }
}

impl Serialize for Timestamp {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut state = serializer.serialize_struct("$timestamp", 1)?;
        //let body = extjson::models::DateTimeBody::from_millis(self.timestamp_millis());
        //state.serialize_field("$timestamp", &body)?;
        state.serialize_field("$timestamp", self)?;
        state.end()
    }
}


