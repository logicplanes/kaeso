use utoipa::openapi::schema::{KnownFormat, SchemaFormat, Type};
use utoipa::openapi::{ObjectBuilder, OneOfBuilder, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use crate::catalogue::Fields;
use crate::decimal::DECIMAL_PATTERN;
use crate::limits::MAX_COUNT;

const PRICE_FIELDS: [&str; 4] = [
    "input_price",
    "output_price",
    "cache_read_price",
    "cache_write_price",
];

fn decimal_or_whole(max: i64, description: &'static str, example: &'static str) -> RefOr<Schema> {
    OneOfBuilder::new()
        .item(
            ObjectBuilder::new()
                .schema_type(Type::String)
                .pattern(Some(DECIMAL_PATTERN)),
        )
        .item(
            ObjectBuilder::new()
                .schema_type(Type::Integer)
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int64)))
                .minimum(Some(0))
                .maximum(Some(max)),
        )
        .description(Some(description))
        .examples([example])
        .into()
}

pub struct LimitAmount;

impl PartialSchema for LimitAmount {
    fn schema() -> RefOr<Schema> {
        decimal_or_whole(
            MAX_COUNT,
            "A non-negative number sent as a decimal string, such as `\"12.50\"`, or as a whole\n\
            JSON number; a fraction sent as a JSON number is refused. Counts (requests and\n\
            tokens) must be whole and at most 10^12; a spend budget is in USD with at most\n\
            9 decimals and at most 1,000,000,000.",
            "50",
        )
    }
}

impl ToSchema for LimitAmount {}

pub struct TokenPrice;

impl PartialSchema for TokenPrice {
    fn schema() -> RefOr<Schema> {
        decimal_or_whole(
            1_000_000,
            "USD per million tokens, from 0 to 1,000,000 with at most 9 decimals, sent as a\n\
            decimal string, such as `\"2.50\"`, or as a whole JSON number; a fraction sent as a\n\
            JSON number is refused.",
            "2.50",
        )
    }
}

impl ToSchema for TokenPrice {}

pub struct FieldsInput;

impl PartialSchema for FieldsInput {
    fn schema() -> RefOr<Schema> {
        let mut schema = Fields::schema();
        if let RefOr::T(Schema::Object(object)) = &mut schema {
            object.description = Some(
                "The fields of a catalogue entry to set. Every field is optional: an override\n\
                sets only what it changes."
                    .to_owned(),
            );
            for name in PRICE_FIELDS {
                if let Some(RefOr::T(Schema::Object(property))) = object.properties.get_mut(name) {
                    let description = property.description.clone();
                    let examples = property.examples.clone();
                    let mut input = OneOfBuilder::new()
                        .item(ObjectBuilder::new().schema_type(Type::Null))
                        .item(RefOr::<Schema>::Ref(
                            utoipa::openapi::Ref::from_schema_name(TokenPrice::name()),
                        ))
                        .build();
                    input.description = description;
                    input.examples = examples;
                    object
                        .properties
                        .insert(name.to_owned(), RefOr::T(Schema::OneOf(input)));
                }
            }
        }
        schema
    }
}

impl ToSchema for FieldsInput {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((TokenPrice::name().into_owned(), TokenPrice::schema()));
        Fields::schemas(schemas);
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Amount {
        #[serde(deserialize_with = "crate::limits::amount")]
        value: Option<rust_decimal::Decimal>,
    }

    #[derive(Deserialize)]
    struct Price {
        #[serde(deserialize_with = "crate::models_dev::price")]
        value: Option<rust_decimal::Decimal>,
    }

    #[test]
    fn what_the_pattern_describes_is_what_the_parsers_accept() {
        for text in [
            "0", "5", "12.50", "0.000001", ".5", "1e3", "2.5E-1", "1.5e+2",
        ] {
            let body = format!(r#"{{"value": "{text}"}}"#);
            assert!(
                serde_json::from_str::<Amount>(&body).is_ok_and(|a| a.value.is_some()),
                "{text}"
            );
            assert!(
                serde_json::from_str::<Price>(&body).is_ok_and(|p| p.value.is_some()),
                "{text}"
            );
        }
        for number in ["0", "5", "1000000"] {
            let body = format!(r#"{{"value": {number}}}"#);
            assert!(
                serde_json::from_str::<Amount>(&body).is_ok_and(|a| a.value.is_some()),
                "{number}"
            );
            assert!(
                serde_json::from_str::<Price>(&body).is_ok_and(|p| p.value.is_some()),
                "{number}"
            );
        }
        for text in ["+5", "5.", "1_000", " 5", "5 ", "-5", ""] {
            let body = format!(r#"{{"value": "{text}"}}"#);
            assert!(serde_json::from_str::<Amount>(&body).is_err(), "{text:?}");
            assert!(serde_json::from_str::<Price>(&body).is_err(), "{text:?}");
        }
        for number in ["0.5", "1e3", "5.0"] {
            let body = format!(r#"{{"value": {number}}}"#);
            assert!(serde_json::from_str::<Amount>(&body).is_err(), "{number}");
            assert!(serde_json::from_str::<Price>(&body).is_err(), "{number}");
        }
    }
}
