use std::process;

#[tokio::main]
async fn main() {
    process::exit(ephyr::run().await);
}
