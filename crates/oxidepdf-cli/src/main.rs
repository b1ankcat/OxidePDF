#[tokio::main]
async fn main() {
    std::process::exit(oxidepdf_cli::run().await);
}
