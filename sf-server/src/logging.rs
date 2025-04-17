use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sf_ice=debug,http_tower=debug".into()),
        )
        .with(
            fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true),
        )
        .init();
}

#[cfg(test)]
mod tests {
    //use super::*;
    //use std::sync::Once;
    //use tracing::dispatcher::Dispatch;
    //static INIT: Once = Once::new();

    //fn setup_test() {
    //    INIT.call_once(|| {
    //        setup_logging();
    //    });
    //}

    //#[test]
    //fn test_logging_initialization() {
    //    setup_test();
    //    tracing::dispatcher::
    //}
}
