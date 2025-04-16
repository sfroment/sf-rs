use std::{fmt::Debug, sync::Arc};

pub type Labels<'a> = &'a [(&'a str, &'a str)];

/// A metric that can be incremented.
pub trait Counter: Send + Sync {
    /// Increment the counter by 1
    fn increment(&self);

    /// Increment the counter by a given value
    fn increment_by(&self, value: f64);

    /// Create a new counter with the given labels
    fn with_labels(&self, labels: Labels) -> Arc<dyn Counter>;

    /// Create a new counter
    fn without_labels(&self) -> Arc<dyn Counter>;
}

/// A metric that can be incremented or decremented.
pub trait Gauge: Send + Sync {
    /// Increment the gauge by 1
    fn increment(&self);

    /// Decrement the gauge by 1
    fn decrement(&self);

    /// Set the gauge to a given value
    fn set(&self, value: f64);

    /// Add a given value to the gauge
    fn add(&self, value: f64);

    /// Subtract a given value from the gauge
    fn subtract(&self, value: f64);

    /// Create a new gauge with the given labels
    fn with_labels(&self, labels: Labels) -> Arc<dyn Gauge>;

    /// Create a new gauge
    fn without_labels(&self) -> Arc<dyn Gauge>;
}

/// A helper trait for Histogram timers. Automatically records duration on drop.
pub trait HistogramTimer: Send + Sync {
    fn observe_duration(&mut self);
}

pub trait Histogram: Send + Sync {
    /// Observe a single value.
    fn observe(&self, v: f64);

    /// Start a timer. When the returned timer object goes out of scope (or `observe_duration` is called),
    /// the elapsed time is recorded in the histogram.
    /// The timer itself should hold the necessary labels.
    fn start_timer(&self) -> Box<dyn HistogramTimer>;

    /// Returns a version of this histogram specific to the given labels.
    fn with_labels(&self, labels: Labels<'_>) -> Arc<dyn Histogram>;

    /// Returns a version of this histogram without labels
    fn without_labels(&self) -> Arc<dyn Histogram>;
}

/// A trait for creating and managing metrics.
pub trait Metrics: Clone + Send + Sync + 'static + Debug {
    /// Type for creating and managing counters.
    type C: Counter + Clone;
    /// Type for creating and managing gauges.
    type G: Gauge + Clone;
    /// Type for creating and managing histograms.
    type H: Histogram + Clone;

    /// Create a new counter with the given name and help text.
    fn counter(&self, name: &str, help: &str) -> Arc<Self::C>;
    /// Create a new gauge with the given name and help text.
    fn gauge(&self, name: &str, help: &str) -> Arc<Self::G>;
    /// Create a new histogram with the given name, help text, and buckets.
    fn histogram(&self, name: &str, help: &str, buckets: Option<&[f64]>) -> Arc<Self::H>;
}
