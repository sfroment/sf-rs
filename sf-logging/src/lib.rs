#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        #[cfg(not(coverage))]
        ::tracing::error!($($arg)+);
        #[cfg(coverage)]
        { /* nothing */ }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        #[cfg(not(coverage))]
        ::tracing::warn!($($arg)+);
        #[cfg(coverage)]
        { /* nothing */ }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        #[cfg(not(coverage))]
        ::tracing::info!($($arg)+);
        #[cfg(coverage)]
        { /* nothing */ }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        #[cfg(not(coverage))]
        ::tracing::debug!($($arg)+);
        #[cfg(coverage)]
        { /* nothing */ }
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {
        #[cfg(not(coverage))]
        ::tracing::trace!($($arg)+);
        #[cfg(coverage)]
        { /* nothing */ }
    };
}
