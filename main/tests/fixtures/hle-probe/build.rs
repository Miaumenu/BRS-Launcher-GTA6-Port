fn main() {
    println!("cargo:rustc-link-arg-cdylib=/DELAYLOAD:sixm-probe-delayed.dll");
    println!("cargo:rustc-link-lib=delayimp");
}
