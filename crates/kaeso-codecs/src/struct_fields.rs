use serde::ser::{Error, Impossible, SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};

pub(crate) fn written_names<T: Serialize + ?Sized>(
    value: &T,
) -> Result<Vec<&'static str>, serde_json::Error> {
    value.serialize(Fields(Names))
}

pub(crate) fn write_into<T: Serialize + ?Sized, M: SerializeMap>(
    value: &T,
    map: &mut M,
) -> Result<Vec<&'static str>, M::Error> {
    value.serialize(Fields(Entries(map)))
}

trait Sink {
    type Error: Error;

    fn field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>;
}

struct Names;

impl Sink for Names {
    type Error = serde_json::Error;

    fn field<T: Serialize + ?Sized>(&mut self, _: &'static str, _: &T) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct Entries<'a, M>(&'a mut M);

impl<M: SerializeMap> Sink for Entries<'_, M> {
    type Error = M::Error;

    fn field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.0.serialize_entry(key, value)
    }
}

struct Fields<S>(S);

struct Collect<S> {
    sink: S,
    names: Vec<&'static str>,
}

impl<S: Sink> SerializeStruct for Collect<S> {
    type Ok = Vec<&'static str>;
    type Error = S::Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.names.push(key);
        self.sink.field(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.names)
    }
}

fn not_a_struct<E: Error>() -> E {
    E::custom("only structs carry extra fields")
}

macro_rules! refuse {
    ($($method:ident($($arg:ty),*) -> $ret:ty;)*) => {
        $(
            fn $method(self, $(_: $arg),*) -> Result<$ret, Self::Error> {
                Err(not_a_struct())
            }
        )*
    };
}

impl<S: Sink> Serializer for Fields<S> {
    type Ok = Vec<&'static str>;
    type Error = S::Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Collect<S>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(Collect {
            sink: self.0,
            names: Vec::with_capacity(len),
        })
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<Self::Ok, Self::Error> {
        Err(not_a_struct())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(not_a_struct())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(not_a_struct())
    }

    refuse! {
        serialize_bool(bool) -> Self::Ok;
        serialize_i8(i8) -> Self::Ok;
        serialize_i16(i16) -> Self::Ok;
        serialize_i32(i32) -> Self::Ok;
        serialize_i64(i64) -> Self::Ok;
        serialize_u8(u8) -> Self::Ok;
        serialize_u16(u16) -> Self::Ok;
        serialize_u32(u32) -> Self::Ok;
        serialize_u64(u64) -> Self::Ok;
        serialize_f32(f32) -> Self::Ok;
        serialize_f64(f64) -> Self::Ok;
        serialize_char(char) -> Self::Ok;
        serialize_str(&str) -> Self::Ok;
        serialize_bytes(&[u8]) -> Self::Ok;
        serialize_none() -> Self::Ok;
        serialize_unit() -> Self::Ok;
        serialize_unit_struct(&'static str) -> Self::Ok;
        serialize_unit_variant(&'static str, u32, &'static str) -> Self::Ok;
        serialize_seq(Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(usize) -> Self::SerializeTuple;
        serialize_tuple_struct(&'static str, usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Self::SerializeTupleVariant;
        serialize_map(Option<usize>) -> Self::SerializeMap;
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Self::SerializeStructVariant;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[derive(Serialize)]
    struct Sample {
        #[serde(rename = "type")]
        ty: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        absent: Option<u32>,
        present: Option<u32>,
        nested: Vec<u32>,
    }

    fn sample() -> Sample {
        Sample {
            ty: "x",
            absent: None,
            present: None,
            nested: vec![1, 2],
        }
    }

    #[test]
    fn names_are_the_keys_serde_writes() {
        assert_eq!(
            written_names(&sample()).unwrap(),
            ["type", "present", "nested"]
        );
    }

    #[test]
    fn fields_are_written_into_the_map() {
        let mut out = Vec::new();
        let mut ser = serde_json::Serializer::new(&mut out);
        let mut map = ser.serialize_map(None).unwrap();
        let names = write_into(&sample(), &mut map).unwrap();
        map.serialize_entry("extra", &true).unwrap();
        SerializeMap::end(map).unwrap();
        assert_eq!(names, ["type", "present", "nested"]);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            r#"{"type":"x","present":null,"nested":[1,2],"extra":true}"#
        );
    }

    #[test]
    fn anything_but_a_struct_is_refused() {
        for value in [
            serde_json::json!({"a": 1}),
            serde_json::json!([1]),
            serde_json::json!("s"),
            serde_json::json!(1),
            serde_json::json!(1.5),
            serde_json::json!(true),
            Value::Null,
        ] {
            assert!(written_names(&value).is_err(), "{value}");
        }
        assert!(written_names(&Some(1)).is_err());
        assert!(written_names(&()).is_err());
        assert!(written_names(&'c').is_err());
        assert!(written_names(&(1, 2)).is_err());
    }

    #[test]
    fn every_other_serializer_entry_point_is_refused() {
        let f = || Fields(Names);
        assert!(f().serialize_i8(1).is_err());
        assert!(f().serialize_i16(1).is_err());
        assert!(f().serialize_i32(1).is_err());
        assert!(f().serialize_u8(1).is_err());
        assert!(f().serialize_u16(1).is_err());
        assert!(f().serialize_u32(1).is_err());
        assert!(f().serialize_f32(1.0).is_err());
        assert!(f().serialize_bytes(b"x").is_err());
        assert!(f().serialize_unit_struct("U").is_err());
        assert!(f().serialize_unit_variant("E", 0, "A").is_err());
        assert!(f().serialize_newtype_struct("N", &1).is_err());
        assert!(f().serialize_newtype_variant("E", 0, "A", &1).is_err());
        assert!(f().serialize_tuple_struct("T", 1).is_err());
        assert!(f().serialize_tuple_variant("E", 0, "A", 1).is_err());
        assert!(f().serialize_struct_variant("E", 0, "A", 1).is_err());
    }
}
