use peligames_addon_kit::{manifest::Manifest, service::serve};
use peligames_optiscaler_service::Service;
use std::io;

fn main() -> io::Result<()> {
    #[cfg(feature = "native-ui")]
    let source = include_str!("../../manifest.native.json");
    #[cfg(not(feature = "native-ui"))]
    let source = include_str!("../../manifest.json");
    let manifest: Manifest =
        serde_json::from_str(source).expect("valid bundled OptiScaler manifest");
    if std::env::args().nth(1).as_deref() == Some("--package-info") {
        println!(
            "{}",
            serde_json::to_string(&manifest).expect("valid package identity")
        );
        return Ok(());
    }
    let mut service = Service::new()?;
    serve(
        io::stdin().lock(),
        io::stdout().lock(),
        &manifest,
        |method, value| service.handle(method, value),
    )
}
