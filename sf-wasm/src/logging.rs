pub fn init_logging() {
    console_error_panic_hook::set_once();
    //console_log::init_with_level(log::Level::Trace).expect("console_log failed");
    tracing_wasm::set_as_global_default();
}
