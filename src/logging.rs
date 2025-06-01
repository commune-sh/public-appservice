use std::process::ExitCode;

use tracing_subscriber::{fmt::Layer, layer::SubscriberExt as _, EnvFilter, Layer as _, Registry};

pub fn init(directives: &str) -> Result<(), ExitCode> {
    let layer = Layer::new().with_writer(std::io::stderr).pretty().boxed();

    let filtered_layer = layer.with_filter(EnvFilter::new(directives));

    let registry = Registry::default().with(filtered_layer);

    tracing::subscriber::set_global_default(registry).map_err(|error| {
        eprintln!("Failed to set tracing subscriber: {error}");

        ExitCode::FAILURE
    })
}
