use serde::{Serialize, de::DeserializeOwned};

/// Merge CLI-provided values over application defaults.
///
/// Any field set to `None` in the `cli` argument will leave the corresponding
/// value from `defaults` intact. This function is intended for simple "CLI over
/// defaults" merging in example code and small projects.
pub fn merge_cli_over_defaults<T>(defaults: T, cli: T) -> T
where
    T: Serialize + DeserializeOwned + Default,
{
    let mut val = serde_json::to_value(defaults).unwrap_or_default();
    let cli_val = serde_json::to_value(cli).unwrap_or_default();
    if let serde_json::Value::Object(cli_map) = cli_val {
        if let serde_json::Value::Object(ref mut base) = val {
            for (k, v) in cli_map {
                if !v.is_null() {
                    base.insert(k, v);
                }
            }
        }
    }
    serde_json::from_value(val).unwrap_or_default()
}
