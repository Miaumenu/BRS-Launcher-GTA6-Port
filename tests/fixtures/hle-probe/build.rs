fn main() {
    println!("cargo:rustc-link-arg-cdylib=/DELAYLOAD:gambiarra-probe-delayed.dll");
    println!("cargo:rustc-link-lib=delayimp");
}
