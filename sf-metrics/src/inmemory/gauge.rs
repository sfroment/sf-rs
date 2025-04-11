use super::common::{LabelKey, MetricStorage};
use crate::Gauge;
use portable_atomic::AtomicF64;
use std::sync::{atomic::Ordering, Arc};

pub(crate) type GaugeStorage = MetricStorage<AtomicF64>;

#[derive(Clone, Debug)]
pub struct InMemoryGauge {
    metrics: Arc<GaugeStorage>,
    value: Arc<AtomicF64>,
}

impl InMemoryGauge {
    pub(crate) fn new(metrics: Arc<GaugeStorage>, labels: LabelKey) -> Self {
        let value = metrics.get_or_create_default(&labels);
        Self { metrics, value }
    }
}

impl Gauge for InMemoryGauge {
    fn add(&self, value: f64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    fn decrement(&self) {
        self.value.fetch_sub(1.0, Ordering::Relaxed);
    }

    fn increment(&self) {
        self.value.fetch_add(1.0, Ordering::Relaxed);
    }

    fn set(&self, value: f64) {
        self.value.store(value, Ordering::Relaxed);
    }

    fn subtract(&self, value: f64) {
        self.value.fetch_sub(value, Ordering::Relaxed);
    }

    fn without_labels(&self) -> Arc<dyn Gauge> {
        let empty_key = LabelKey::empty();
        Arc::new(InMemoryGauge::new(Arc::clone(&self.metrics), empty_key))
    }

    fn with_labels(&self, labels: crate::Labels) -> Arc<dyn Gauge> {
        let new_key = LabelKey::new(labels);
        Arc::new(InMemoryGauge::new(Arc::clone(&self.metrics), new_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryMetrics, Metrics};

    #[test]
    fn test_gauge() {
        let provider = InMemoryMetrics::new();
        let gauge = provider.gauge("test_gauge", "A test gauge");
        let labeled_gauge = gauge.with_labels(&[("dim", "1")]);

        gauge.set(10.0);
        assert_eq!(provider.get_gauge_value("test_gauge", &[]), Some(10.0));
        gauge.increment(); // 11.0
        gauge.decrement(); // 10.0
        gauge.add(5.0); // 15.0
        gauge.subtract(3.0); // 12.0
        assert_eq!(provider.get_gauge_value("test_gauge", &[]), Some(12.0));

        labeled_gauge.set(100.0);
        assert_eq!(
            provider.get_gauge_value("test_gauge", &[("dim", "1")]),
            Some(100.0)
        );
        assert_eq!(provider.get_gauge_value("test_gauge", &[]), Some(12.0));
    }

    #[test]
    fn test_gauge_without_labels() {
        let provider = InMemoryMetrics::new();
        let gauge = provider.gauge("test_gauge", "A test gauge");
        let labeled_gauge = gauge.with_labels(&[("dim", "1")]);
        let unlabeled_gauge = labeled_gauge.without_labels();

        labeled_gauge.set(100.0);
        unlabeled_gauge.set(50.0);

        assert_eq!(
            provider.get_gauge_value("test_gauge", &[("dim", "1")]),
            Some(100.0)
        );
        assert_eq!(provider.get_gauge_value("test_gauge", &[]), Some(50.0));

        unlabeled_gauge.increment();
        assert_eq!(provider.get_gauge_value("test_gauge", &[]), Some(51.0));
        assert_eq!(
            provider.get_gauge_value("test_gauge", &[("dim", "1")]),
            Some(100.0)
        );
    }
}
