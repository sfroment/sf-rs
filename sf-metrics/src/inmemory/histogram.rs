use super::common::{LabelKey, MetricStorage};
use crate::{Histogram, HistogramTimer, Labels};
use portable_atomic::{AtomicF64, AtomicU64};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Instant,
};
use tracing::warn;

#[derive(Debug)]
pub(crate) struct HistogramStorage {
    pub(crate) buckets: Vec<f64>,
    pub(crate) storage: MetricStorage<HistogramAtomics>,
}

impl HistogramStorage {
    pub(crate) fn new(input_buckets: Option<&[f64]>) -> Self {
        let default_buckets = vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ];
        let mut buckets_vec = input_buckets.map_or_else(|| default_buckets, |s| s.to_vec());
        buckets_vec.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        buckets_vec.retain(|b| b.is_finite());
        buckets_vec.dedup();

        Self {
            buckets: buckets_vec,
            storage: MetricStorage::new(),
        }
    }

    pub(crate) fn get_or_create(&self, key: &LabelKey) -> Arc<HistogramAtomics> {
        self.storage
            .data
            .entry(key.clone())
            .or_insert_with(|| Arc::new(HistogramAtomics::new(self.buckets.len())))
            .clone()
    }
}

#[derive(Clone, Debug)]
pub struct InMemoryHistogram {
    metrics: Arc<HistogramStorage>,
    atomics: Arc<HistogramAtomics>,
}

impl InMemoryHistogram {
    pub(crate) fn new(metrics: Arc<HistogramStorage>, labels: LabelKey) -> Self {
        let atomics = metrics.get_or_create(&labels);
        Self { metrics, atomics }
    }
}

impl Histogram for InMemoryHistogram {
    fn observe(&self, v: f64) {
        if !v.is_finite() {
            warn!("Attempted to observe non-finite value in histogram: {}", v);
            return;
        }

        let bucket_index = self
            .metrics
            .buckets
            .binary_search_by(|probe| probe.partial_cmp(&v).unwrap_or(std::cmp::Ordering::Less))
            .unwrap_or_else(|insertion_point| insertion_point);

        // Increment bucket count, sum, and total count using the direct atomics reference.
        if let Some(bucket_counter) = self.atomics.bucket_counts.get(bucket_index) {
            bucket_counter.fetch_add(1, Ordering::Relaxed);
        } else {
            // This should technically not happen if index calculation is correct,
            // as bucket_counts has length buckets.len() + 1.
            // The highest index should be buckets.len(), hitting the +Inf bucket.
            #[cfg(not(tarpaulin_include))]
            {
                warn!(
                    "Calculated histogram bucket index {} out of bounds ({} buckets). Value: {}",
                    bucket_index,
                    self.metrics.buckets.len(),
                    v
                );
                let last = self.atomics.bucket_counts.last();
                if let Some(inf_bucket) = last {
                    inf_bucket.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        self.atomics.sum.fetch_add(v, Ordering::Relaxed);
        self.atomics.count.fetch_add(1, Ordering::Relaxed);
    }

    fn start_timer(&self) -> Box<dyn HistogramTimer> {
        // Clone the Arc<InMemoryHistogram> to be owned by the timer
        Box::new(InMemoryHistogramTimer::new(Arc::new(self.clone())))
    }

    fn with_labels(&self, labels: Labels) -> Arc<dyn Histogram> {
        let new_key = LabelKey::new(labels);
        Arc::new(InMemoryHistogram::new(Arc::clone(&self.metrics), new_key))
    }

    fn without_labels(&self) -> Arc<dyn Histogram> {
        let empty_key = LabelKey::empty();
        Arc::new(InMemoryHistogram::new(Arc::clone(&self.metrics), empty_key))
    }
}

struct InMemoryHistogramTimer {
    start: Instant,
    histogram: Arc<InMemoryHistogram>,
    // Use a flag to prevent double-recording if observe_duration is called explicitly.
    observed: bool,
}

impl InMemoryHistogramTimer {
    fn new(histogram: Arc<InMemoryHistogram>) -> Self {
        Self {
            start: Instant::now(),
            histogram,
            observed: false,
        }
    }

    /// Perform the observation.
    fn do_observe(&mut self) {
        if !self.observed {
            let duration = self.start.elapsed().as_secs_f64();
            self.histogram.observe(duration);
            self.observed = true;
        }
    }
}

impl HistogramTimer for InMemoryHistogramTimer {
    /// Explicitly observe and consume the timer.
    fn observe_duration(&mut self) {
        self.do_observe();
    }
}

impl Drop for InMemoryHistogramTimer {
    /// Observe duration automatically if `observe_duration` wasn't called.
    fn drop(&mut self) {
        self.do_observe();
    }
}

#[derive(Debug)]
pub(crate) struct HistogramAtomics {
    pub(crate) bucket_counts: Vec<AtomicU64>,
    pub(crate) sum: AtomicF64,
    pub(crate) count: AtomicU64,
}

impl HistogramAtomics {
    fn new(num_buckets: usize) -> Self {
        HistogramAtomics {
            bucket_counts: (0..=num_buckets).map(|_| AtomicU64::new(0)).collect(),
            sum: AtomicF64::new(0.0),
            count: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Histogram, InMemoryMetrics, Metrics};
    use std::{sync::Arc, thread};

    #[test]
    fn test_histogram() {
        let provider = InMemoryMetrics::new();
        // Buckets: <=0.1, <=0.5, <=1.0, +Inf
        let buckets = &[0.1, 0.5, 1.0];
        let histogram = provider.histogram("test_hist", "A test histogram", Some(buckets));
        let labeled_hist = histogram.with_labels(&[("type", "a")]);

        histogram.observe(0.05); // Bucket 0 (<=0.1)
        histogram.observe(0.3); // Bucket 1 (<=0.5)
        histogram.observe(0.8); // Bucket 2 (<=1.0)
        histogram.observe(5.0); // Bucket 3 (+Inf)
        histogram.observe(0.11); // Bucket 1 (<=0.5)

        labeled_hist.observe(0.6); // Bucket 2 (<=1.0)

        let (bounds, counts, sum, count) = provider.get_histogram_values("test_hist", &[]).unwrap();
        assert_eq!(bounds, vec![0.1, 0.5, 1.0]);
        // Counts: 1 (<=0.1), 2 (<=0.5), 1 (<=1.0), 1 (+Inf)
        assert_eq!(counts, vec![1, 2, 1, 1]);
        assert!((sum - (0.05 + 0.3 + 0.8 + 5.0 + 0.11)).abs() < f64::EPSILON);
        assert_eq!(count, 5);

        let (bounds_l, counts_l, sum_l, count_l) = provider
            .get_histogram_values("test_hist", &[("type", "a")])
            .unwrap();
        assert_eq!(bounds_l, vec![0.1, 0.5, 1.0]);
        assert_eq!(counts_l, vec![0, 0, 1, 0]);
        assert!((sum_l - 0.6).abs() < f64::EPSILON);
        assert_eq!(count_l, 1);
    }

    #[test]
    fn test_histogram_timer() {
        let provider = Arc::new(InMemoryMetrics::new());
        let buckets = &[0.1, 0.5];
        let histogram = provider.histogram("timer_hist", "Timer test", Some(buckets));

        {
            let _timer = histogram.start_timer();
            thread::sleep(std::time::Duration::from_millis(150));
            // Timer drops here, observes duration via Drop
        }

        let (_, counts, _, count) = provider.get_histogram_values("timer_hist", &[]).unwrap();
        assert_eq!(counts[0], 0); // <= 0.1
        assert_eq!(counts[1], 1); // <= 0.5
        assert_eq!(counts[2], 0); // +Inf
        assert_eq!(count, 1);

        let mut timer = histogram.start_timer();

        timer.observe_duration();

        // Drop should not observe again because the flag is set
        drop(timer);

        thread::sleep(std::time::Duration::from_millis(10));

        let (_, counts_after, _, count_after) =
            provider.get_histogram_values("timer_hist", &[]).unwrap();
        assert_eq!(counts_after[0], 1); // <= 0.1 (from the observe_now call)
        assert_eq!(counts_after[1], 1); // <= 0.5 (from the first timer drop)
        assert_eq!(counts_after[2], 0); // +Inf
        assert_eq!(count_after, 2); // Total count = 2
    }

    #[test]
    fn test_histogram_warnings() {
        let provider = InMemoryMetrics::new();
        let buckets = &[0.1, 0.5, 1.0];
        let histogram = provider.histogram("warn_hist", "Warning test", Some(buckets));

        histogram.observe(f64::NAN);
        histogram.observe(f64::INFINITY);
        histogram.observe(f64::NEG_INFINITY);

        let (_, counts, _, count) = provider.get_histogram_values("warn_hist", &[]).unwrap();
        assert_eq!(counts, vec![0, 0, 0, 0]); // Non-finite values are rejected
        assert_eq!(count, 0);

        histogram.observe(2.0); // Value larger than all buckets
        let (_, counts_after, sum_after, count_after) =
            provider.get_histogram_values("warn_hist", &[]).unwrap();
        assert_eq!(counts_after, vec![0, 0, 0, 1]); // Value goes to +Inf bucket
        assert!((sum_after - 2.0).abs() < f64::EPSILON);
        assert_eq!(count_after, 1);

        let value = 1.0 + f64::EPSILON; // Value just above the last bucket
        histogram.observe(value);
        let (_, counts_error, sum_error, count_error) =
            provider.get_histogram_values("warn_hist", &[]).unwrap();
        assert_eq!(counts_error, vec![0, 0, 0, 2]); // Both large values go to +Inf bucket
        assert!((sum_error - (2.0 + value)).abs() < f64::EPSILON);
        assert_eq!(count_error, 2);
    }

    #[test]
    fn test_histogram_without_labels() {
        let provider = InMemoryMetrics::new();
        let buckets = &[0.1, 0.5, 1.0];
        let histogram = provider.histogram("label_hist", "Label test", Some(buckets));

        let labeled_hist = histogram.with_labels(&[("type", "a")]);
        let unlabeled_hist = labeled_hist.without_labels();

        labeled_hist.observe(0.2);
        unlabeled_hist.observe(0.3);

        let (_, counts_l, sum_l, count_l) = provider
            .get_histogram_values("label_hist", &[("type", "a")])
            .unwrap();
        assert_eq!(counts_l, vec![0, 1, 0, 0]); // 0.2 falls in bucket 1 (<=0.5)
        assert!((sum_l - 0.2).abs() < f64::EPSILON);
        assert_eq!(count_l, 1);

        let (_, counts_u, sum_u, count_u) =
            provider.get_histogram_values("label_hist", &[]).unwrap();
        assert_eq!(counts_u, vec![0, 1, 0, 0]); // 0.3 falls in bucket 1 (<=0.5)
        assert!((sum_u - 0.3).abs() < f64::EPSILON);
        assert_eq!(count_u, 1);
    }
}
