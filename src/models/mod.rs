//! Typed models generated from Truto's OpenAPI spec.
//!
//! All structs decode leniently: unknown fields are ignored and missing
//! fields take their default. Enums are open (unknown values land in
//! `Other(String)` and serialise back unchanged), and unions keep the raw
//! JSON with an `as_<variant>()` accessor per variant.

#[allow(missing_docs)]
#[rustfmt::skip]
mod generated;

pub use generated::*;
