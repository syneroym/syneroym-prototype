use syneroym_common::config::Config;

fn main() {
    divan::main();
}

#[divan::bench]
fn config_default() {
    divan::black_box(Config::default());
}