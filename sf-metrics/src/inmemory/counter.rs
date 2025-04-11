use super::common::{LabelKey, MetricStorage};
use crate::Counter;
use portable_atomic::AtomicF64;
use std::sync::{atomic::Ordering, Arc};
use tracing::warn;

pub(crate) type CounterStorage = MetricStorage<AtomicF64>;

#[derive(Clone, Debug)]
pub struct InMemoryCounter {
    metrics: Arc<CounterStorage>,
    value: Arc<AtomicF64>,
}

impl InMemoryCounter {
    pub(crate) fn new(metrics: Arc<CounterStorage>, labels: LabelKey) -> Self {
        let value = metrics.get_or_create_default(&labels);
        Self { metrics, value }
    }
}

impl Counter for InMemoryCounter {
    fn increment(&self) {
        self.value.fetch_add(1.0, Ordering::Relaxed);
    }

    fn increment_by(&self, v: f64) {
        if v < 0.0 {
            warn!("Attempted to increment counter by a negative value: {}", v);
            return;
        }
        self.value.fetch_add(v, Ordering::Relaxed);
    }

    fn with_labels(&self, labels: crate::Labels) -> Arc<dyn Counter> {
        let new_key = LabelKey::new(labels);
        Arc::new(InMemoryCounter::new(Arc::clone(&self.metrics), new_key))
    }

    fn without_labels(&self) -> Arc<dyn Counter> {
        let empty_key = LabelKey::empty();
        Arc::new(InMemoryCounter::new(Arc::clone(&self.metrics), empty_key))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Counter, InMemoryMetrics, Metrics};
    use tracing_test::traced_test;

    #[test]
    fn test_counter_basic() {
        let provider = InMemoryMetrics::new();
        let counter = provider.counter("test_count", "A test counter");

        counter.increment();
        counter.increment_by(5.0);

        assert_eq!(provider.get_counter_value("test_count", &[]), Some(6.0));
        assert_eq!(
            provider.get_counter_value("test_count", &[("a", "1")]),
            None
        );
    }

    #[test]
    fn test_counter_labels() {
        let provider = InMemoryMetrics::new();
        let counter = provider.counter("test_labeled_count", "A test counter");

        let labels1 = &[("method", "GET"), ("status", "200")];
        let labels2 = &[("method", "POST"), ("status", "500")];
        let labels1_sorted = &[("method", "GET"), ("status", "200")];
        let labels2_sorted = &[("method", "POST"), ("status", "500")];

        counter.with_labels(labels1).increment();
        counter.with_labels(labels1).increment_by(2.0);
        counter.with_labels(labels2).increment();

        counter.increment();

        assert_eq!(
            provider.get_counter_value("test_labeled_count", labels1_sorted),
            Some(3.0)
        );
        assert_eq!(
            provider.get_counter_value("test_labeled_count", labels2_sorted),
            Some(1.0)
        );
        assert_eq!(
            provider.get_counter_value("test_labeled_count", &[]),
            Some(1.0)
        );

        let labeled = counter.with_labels(labels1);
        labeled.increment();
        assert_eq!(
            provider.get_counter_value("test_labeled_count", labels1_sorted),
            Some(4.0)
        );
        let unlabeled_again = labeled.without_labels();
        unlabeled_again.increment();
        assert_eq!(
            provider.get_counter_value("test_labeled_count", &[]),
            Some(2.0)
        );
    }

    #[test]
    #[traced_test]
    fn test_counter_negative_increment() {
        let provider = InMemoryMetrics::new();
        let counter = provider.counter("test_negative_count", "A test counter");

        counter.increment();
        assert_eq!(
            provider.get_counter_value("test_negative_count", &[]),
            Some(1.0)
        );

        counter.increment_by(-5.0);

        assert_eq!(
            provider.get_counter_value("test_negative_count", &[]),
            Some(1.0)
        );

        assert!(logs_contain(
            "Attempted to increment counter by a negative value: -5"
        ));
    }
}
