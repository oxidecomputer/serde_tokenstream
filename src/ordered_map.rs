// Copyright 2022 Oxide Computer Company

use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize};

/// A container to deserialize maps into if the keys are non-unique, or don't
/// implement `Hash` or `Ord`.
///
/// This is a simple wrapper around a `Vec` of key-value pairs. It doesn't
/// check for uniqueness of keys, so it's possible to have multiple values for
/// the same key.
///
/// Because this map lacks any trait requirements, directly looking up keys is
/// not possible. To extract data, call `.into_iter()` on it.
pub struct OrderedMap<K, V> {
    items: Vec<(K, V)>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self { items: Default::default() }
    }
}

impl<'de, K: Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de>
    for OrderedMap<K, V>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(OrderedMapVisitor(PhantomData))
    }
}

struct OrderedMapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K: Deserialize<'de>, V: Deserialize<'de>> Visitor<'de>
    for OrderedMapVisitor<K, V>
{
    type Value = OrderedMap<K, V>;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("a map of key-value pairs")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut items = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some(entry) = map.next_entry()? {
            items.push(entry)
        }
        Ok(OrderedMap { items })
    }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);

    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use serde::Deserialize;

    use crate::{from_tokenstream, Result};

    use super::OrderedMap;

    // Note that critically this isn't Ord, or Hash, or whatever so could not be
    // used in a HashMap or BTreeMap.
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Value(pub String);

    #[test]
    fn test_ordered_map() -> Result<()> {
        let data = from_tokenstream::<OrderedMap<Value, Value>>(&quote! {
            "key" = "value1",
            "key" = "value2"
        })?;

        let mut kv = data.into_iter().map(|(k, v)| (k.0, v.0));
        assert_eq!(kv.next(), Some(("key".to_string(), "value1".to_string())));
        assert_eq!(kv.next(), Some(("key".to_string(), "value2".to_string())));
        assert_eq!(kv.next(), None);

        Ok(())
    }
}
