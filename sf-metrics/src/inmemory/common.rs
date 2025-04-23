use crate::Labels;
use dashmap::DashMap;
use std::{
    fmt::{Debug, Formatter, Result},
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

/// A representation of a label set, sorted by key for consistent hashing.
#[derive(Clone, Eq, Ord, PartialOrd)]
pub(crate) struct LabelKey(Arc<Vec<(String, String)>>);

impl PartialEq for LabelKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Debug for LabelKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:?}", self.0)
    }
}

impl Hash for LabelKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl LabelKey {
    pub(crate) fn new(labels: Labels) -> Self {
        if labels.is_empty() {
            return Self::empty();
        }

        let mut owned_labels: Vec<(String, String)> = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        owned_labels.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        LabelKey(Arc::new(owned_labels))
    }

    pub(crate) fn empty() -> Self {
        static EMPTY_LABELS: OnceLock<LabelKey> = OnceLock::new();
        EMPTY_LABELS
            .get_or_init(|| LabelKey(Arc::new(Vec::new())))
            .clone()
    }

    pub(crate) fn labels(&self) -> &[(String, String)] {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct MetricStorage<T: Send + Sync + 'static> {
    pub(crate) data: DashMap<LabelKey, Arc<T>>,
}

impl<T: Send + Sync + 'static> MetricStorage<T> {
    pub(crate) fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }
}

impl<T: Send + Sync + Default + 'static> MetricStorage<T> {
    pub(crate) fn get_or_create_default(&self, key: &LabelKey) -> Arc<T> {
        self.data
            .entry(key.clone())
            .or_insert_with(|| Arc::new(T::default()))
            .clone()
    }
}

pub(crate) fn format_labels(labels: &[(String, String)]) -> String {
    labels
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::{format_labels, LabelKey};
    use std::{collections::HashMap, sync::Arc};

    #[test]
    fn test_label_key_equality() {
        let l1 = LabelKey::new(&[("a", "1"), ("b", "2")]);
        let l2 = LabelKey::new(&[("b", "2"), ("a", "1")]);
        assert_eq!(l1, l2);
    }

    #[test]
    fn test_empty_label_key() {
        let l_empty1 = LabelKey::new(&[]);
        let l_empty2 = LabelKey::empty();
        assert_eq!(l_empty1, l_empty2);

        assert!(Arc::ptr_eq(&l_empty1.0, &l_empty2.0));
    }

    #[test]
    fn test_label_key_hashing() {
        let l1 = LabelKey::new(&[("a", "1"), ("b", "2")]);
        let l2 = LabelKey::new(&[("b", "2"), ("a", "1")]);

        let mut map = HashMap::new();
        map.insert(l1, "value");
        assert!(map.contains_key(&l2));
        assert_eq!(map.get(&l2), Some(&"value"));
    }

    #[test]
    fn test_format_labels_basic() {
        assert_eq!(
            format_labels(&[
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string())
            ]),
            "a=\"1\",b=\"2\""
        );
    }

    #[test]
    fn test_format_labels_escaping() {
        assert_eq!(
            format_labels(&[("key".to_string(), "val\"ue".to_string())]),
            "key=\"val\\\"ue\""
        );
        assert_eq!(
            format_labels(&[("path".to_string(), "C:\\folder".to_string())]),
            "path=\"C:\\\\folder\""
        );
    }

    #[test]
    fn test_format_labels_empty() {
        assert_eq!(format_labels(&[]), "");
    }

    #[test]
    fn test_label_key_debug() {
        let label_key = LabelKey::new(&[("a", "1"), ("b", "2")]);
        let debug_output = format!("{:?}", label_key);

        assert!(debug_output.contains("a"));
        assert!(debug_output.contains("1"));
        assert!(debug_output.contains("b"));
        assert!(debug_output.contains("2"));
    }
}
