fn main() {
    println!("cargo:rustc-link-arg=/ENTRY:start");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
    println!("cargo:rustc-link-arg=/NODEFAULTLIB");
    println!("cargo:rustc-link-arg=/FIXED:NO");
    println!("cargo:rustc-link-arg=/DYNAMICBASE");
    println!("cargo:rustc-link-lib=kernel32");
}
