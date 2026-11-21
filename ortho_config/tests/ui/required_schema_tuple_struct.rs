//! Requirement enumeration names each value by the field it is read from. A
//! tuple struct has no field names, so there is no key to report and the derive
//! must refuse the type rather than invent one.

use ortho_config::OrthoConfig;

/// A tuple struct carries no configuration keys.
#[derive(OrthoConfig)]
struct Root(String);

fn main() {}