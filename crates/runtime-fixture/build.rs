fn main() {
    println!("cargo:rerun-if-changed=../../tests/fixtures/runtime-fixture.ld");
    println!("cargo:rerun-if-changed=../../tests/fixtures/runtime-fixture.S");
    println!("cargo:rustc-link-arg=-Ttests/fixtures/runtime-fixture.ld");
}
