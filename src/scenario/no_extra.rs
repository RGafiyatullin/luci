use serde::de::{self, Visitor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct NoExtra;

impl<'de> Deserialize<'de> for NoExtra {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = NoExtra;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("no extra fields")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                if let Some((key, _value)) = map.next_entry::<String, de::IgnoredAny>()? {
                    Err(de::Error::unknown_field(&key, &[]))
                } else {
                    Ok(NoExtra)
                }
            }
        }

        deserializer.deserialize_map(V)
    }
}
