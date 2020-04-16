use std::process;

#[inline]
fn main() {
    if let Err(e) = ephyr::run() {
        eprintln!("{}", e);
        process::exit(1);
    }
}
