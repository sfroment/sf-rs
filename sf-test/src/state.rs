use sf_metrics::Metrics;

pub(crate) struct AppState<M: Metrics> {
    #[allow(dead_code)]
    metrics: M,
}

impl<M: Metrics> AppState<M> {
    pub fn new(metrics: M) -> Self {
        Self { metrics }
    }
}
