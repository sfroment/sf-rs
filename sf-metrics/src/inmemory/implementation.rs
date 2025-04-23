use super::{
    common::{format_labels, LabelKey},
    counter::{CounterStorage, InMemoryCounter},
    gauge::{GaugeStorage, InMemoryGauge},
    histogram::{HistogramStorage, InMemoryHistogram},
};
use crate::{Labels, Metrics};
use dashmap::DashMap;
use std::sync::{atomic::Ordering, Arc};
use tracing::warn;

#[derive(Clone, Debug, Default)]
pub struct InMemoryMetrics {
    counters: Arc<DashMap<String, Arc<CounterStorage>>>,
    gauges: Arc<DashMap<String, Arc<GaugeStorage>>>,
    histograms: Arc<DashMap<String, Arc<HistogramStorage>>>,
}

impl InMemoryMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_counter_value(&self, name: &str, labels: Labels<'_>) -> Option<f64> {
        self.counters.get(name).and_then(|metric| {
            let key = LabelKey::new(labels);
            metric.data.get(&key).map(|v| v.load(Ordering::Relaxed))
        })
    }

    pub fn get_gauge_value(&self, name: &str, labels: Labels<'_>) -> Option<f64> {
        self.gauges.get(name).and_then(|metric| {
            let key = LabelKey::new(labels);
            metric.data.get(&key).map(|v| v.load(Ordering::Relaxed))
        })
    }

    pub fn get_histogram_values(
        &self,
        name: &str,
        labels: Labels<'_>,
    ) -> Option<(Vec<f64>, Vec<u64>, f64, u64)> {
        self.histograms.get(name).and_then(|metric| {
            let key = LabelKey::new(labels);
            metric.storage.data.get(&key).map(|atomics| {
                let bucket_counts = atomics
                    .bucket_counts
                    .iter()
                    .map(|c| c.load(Ordering::Relaxed))
                    .collect();
                let sum = atomics.sum.load(Ordering::Relaxed);
                let count = atomics.count.load(Ordering::Relaxed);
                (metric.buckets.clone(), bucket_counts, sum, count)
            })
        })
    }

    pub fn gather_metrics_string(&self) -> String {
        let mut output = String::new();
        let ordering = Ordering::Relaxed;

        output.push_str("# Counters\n");
        for entry in self.counters.iter() {
            let name = entry.key();
            let metric = entry.value();
            for labeled_entry in metric.data.iter() {
                let labels_vec = labeled_entry.key().labels();
                let value = labeled_entry.value().load(ordering);
                output.push_str(&format!(
                    "{}{{{}}} {}\n",
                    name,
                    format_labels(labels_vec),
                    value
                ));
            }
        }

        output.push_str("\n# Gauges\n");
        for entry in self.gauges.iter() {
            let name = entry.key();
            let metric = entry.value();
            for labeled_entry in metric.data.iter() {
                let labels_vec = labeled_entry.key().labels();
                let value = labeled_entry.value().load(ordering);
                output.push_str(&format!(
                    "{}{{{}}} {}\n",
                    name,
                    format_labels(labels_vec),
                    value
                ));
            }
        }

        output.push_str("\n# Histograms\n");
        for entry in self.histograms.iter() {
            let name = entry.key();
            let metric = entry.value(); // This is Arc<HistogramMetric>
            for labeled_entry in metric.storage.data.iter() {
                // Iterate through labeled instances
                let labels_vec = labeled_entry.key().labels();
                let labels_str = format_labels(labels_vec);
                let atomics = labeled_entry.value(); // This is Arc<HistogramAtomics>

                let count = atomics.count.load(ordering);
                let sum = atomics.sum.load(ordering);

                output.push_str(&format!("{}_count{{{}}} {}\n", name, labels_str, count));
                output.push_str(&format!("{}_sum{{{}}} {}\n", name, labels_str, sum));

                for (i, boundary) in metric.buckets.iter().enumerate() {
                    let bucket_count = atomics.bucket_counts[i].load(ordering);
                    let label_part = if labels_str.is_empty() {
                        String::new()
                    } else {
                        format!(",{}", labels_str)
                    };
                    output.push_str(&format!(
                        "{}_bucket{{le=\"{}\"{}}} {}\n",
                        name, boundary, label_part, bucket_count
                    ));
                }
                // +Inf bucket
                let inf_bucket_count = atomics
                    .bucket_counts
                    .last()
                    .map(|c| c.load(ordering))
                    .unwrap_or(0);
                let label_part = if labels_str.is_empty() {
                    String::new()
                } else {
                    format!(",{}", labels_str)
                };
                output.push_str(&format!(
                    "{}_bucket{{le=\"+Inf\"{}}} {}\n",
                    name, label_part, inf_bucket_count
                ));
            }
        }

        output
    }
}

impl Metrics for InMemoryMetrics {
    type C = InMemoryCounter;
    type G = InMemoryGauge;
    type H = InMemoryHistogram;

    fn counter(&self, name: &str, _help: &str) -> Arc<Self::C> {
        let metrics = self
            .counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(CounterStorage::new()))
            .clone();

        Arc::new(InMemoryCounter::new(metrics, LabelKey::empty()))
    }

    fn gauge(&self, name: &str, _help: &str) -> Arc<Self::G> {
        let metrics = self
            .gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(GaugeStorage::new()))
            .clone();

        Arc::new(InMemoryGauge::new(metrics, LabelKey::empty()))
    }

    fn histogram(&self, name: &str, _help: &str, buckets: Option<&[f64]>) -> Arc<Self::H> {
        let metric = self
            .histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(HistogramStorage::new(buckets)))
            .clone();

        if let Some(provided_buckets) = buckets {
            if provided_buckets.len() != metric.buckets.len()
                || !provided_buckets
                    .iter()
                    .zip(metric.buckets.iter())
                    .all(|(a, b)| (a - b).abs() < f64::EPSILON)
            {
                warn!("Histogram '{}' already registered with different buckets {:?}. Using existing buckets {:?}.", name, provided_buckets, metric.buckets);
            }
        }

        Arc::new(InMemoryHistogram::new(metric, LabelKey::empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Counter, Gauge, Histogram, InMemoryMetrics};
    use std::{sync::Arc, thread};

    #[test]
    fn test_idempotency() {
        let provider = InMemoryMetrics::new();
        let c1 = provider.counter("idem_counter", "c");
        let c2 = provider.counter("idem_counter", "c");
        c1.increment();
        c2.increment();
        assert_eq!(provider.get_counter_value("idem_counter", &[]), Some(2.0));

        let h1 = provider.histogram("idem_hist", "h", Some(&[1.0, 2.0]));
        let h2 = provider.histogram("idem_hist", "h", Some(&[5.0]));
        h1.observe(1.5);
        h2.observe(0.5);

        let (buckets, counts, _, count) = provider.get_histogram_values("idem_hist", &[]).unwrap();
        assert_eq!(buckets, vec![1.0, 2.0]);

        assert_eq!(counts, vec![1, 1, 0]);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_concurrency() {
        let provider = Arc::new(InMemoryMetrics::new());
        let counter = provider.counter("mt_counter", "mt");
        let gauge = provider.gauge("mt_gauge", "mt");
        let histogram = provider.histogram("mt_hist", "mt", Some(&[0.01, 0.1, 1.0]));

        let num_threads = 10;
        let ops_per_thread = 1000;

        let mut handles = vec![];

        for i in 0..num_threads {
            let counter_clone = Arc::clone(&counter);
            let gauge_clone = Arc::clone(&gauge);
            let histogram_clone = Arc::clone(&histogram);

            let handle = thread::spawn(move || {
                let thread_id_str = i.to_string();
                let labels = &[("thread_id", thread_id_str.as_str())];
                let labeled_counter = counter_clone.with_labels(labels);
                let labeled_gauge = gauge_clone.with_labels(labels);
                let labeled_hist = histogram_clone.with_labels(labels);

                for j in 0..ops_per_thread {
                    labeled_counter.increment();
                    labeled_gauge.set(j as f64);
                    labeled_hist.observe(j as f64 * 0.001);

                    if j % 10 == 0 {
                        counter_clone.increment();
                        gauge_clone.set(i as f64);
                        histogram_clone.observe(i as f64 * 0.1);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify labeled counters
        for i in 0..num_threads {
            let thread_id_str = i.to_string();
            let labels = &[("thread_id", thread_id_str.as_str())];
            let val = provider
                .get_counter_value("mt_counter", labels)
                .unwrap_or(0.0);
            assert_eq!(
                val, ops_per_thread as f64,
                "Counter failed for thread {}",
                i
            );

            let gauge_val = provider.get_gauge_value("mt_gauge", labels).unwrap_or(-1.0);
            assert_eq!(
                gauge_val,
                (ops_per_thread - 1) as f64,
                "Gauge failed for thread {}",
                i
            );

            let (_, _, _, hist_count) = provider
                .get_histogram_values("mt_hist", labels)
                .unwrap_or_default();
            assert_eq!(
                hist_count, ops_per_thread as u64,
                "Histogram count failed for thread {}",
                i
            );
        }

        let unlabeled_counter_val = provider.get_counter_value("mt_counter", &[]).unwrap_or(0.0);
        assert_eq!(
            unlabeled_counter_val,
            (ops_per_thread / 10 * num_threads) as f64
        );

        let unlabeled_gauge_val = provider.get_gauge_value("mt_gauge", &[]).unwrap_or(-1.0);
        assert!(
            (0..num_threads).contains(&(unlabeled_gauge_val as usize)),
            "Unlabeled gauge value {} out of range",
            unlabeled_gauge_val
        );

        let (_, _, _, unlabeled_hist_count) = provider
            .get_histogram_values("mt_hist", &[])
            .unwrap_or_default();
        assert_eq!(
            unlabeled_hist_count,
            (ops_per_thread / 10 * num_threads) as u64
        );

        println!("\n--- Gathered Metrics (Concurrent Test) ---");
        println!("{}", provider.gather_metrics_string());
        println!("--- End Gathered Metrics ---");
    }
}
