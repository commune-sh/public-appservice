use tracing_subscriber::{fmt::Layer, layer::SubscriberExt as _, EnvFilter, Layer as _, Registry};

use crate::error::startup::Main as Error;

pub fn init(directives: &str) -> Result<(), Error> {
    let layer = Layer::new().with_writer(std::io::stderr).pretty().boxed();

    let filtered_layer = layer.with_filter(EnvFilter::new(directives));

    let registry = Registry::default().with(filtered_layer);

    tracing::subscriber::set_global_default(registry).map_err(Error::Tracing)
}
