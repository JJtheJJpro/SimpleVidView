fn main() {
    println!("cargo:rustc-link-lib=delayimp");

    let mut res = winres::WindowsResource::new();
    res.set_icon("logo.ico");
    res.compile().unwrap();
}