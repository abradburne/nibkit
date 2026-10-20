//! Which system nibkit is built for, as one cfg the code reads: `nibkit_desktop` (feature
//! `desktop`, or any target that isn't Linux, or a Linux build that asks for no system),
//! `nibkit_nibos` (feature `nibos`, the default) or `nibkit_stock` (feature `stock`).

fn main() {
    println!("cargo::rustc-check-cfg=cfg(nibkit_desktop, nibkit_nibos, nibkit_stock)");
    let feature = |name: &str| std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some();
    let linux = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux");
    let system = if feature("DESKTOP") || !linux {
        "desktop"
    } else if feature("NIBOS") && feature("STOCK") {
        panic!("nibkit: build for one system: feature `nibos` (the default), `stock` or `desktop`");
    } else if feature("NIBOS") {
        "nibos"
    } else if feature("STOCK") {
        "stock"
    } else {
        "desktop"
    };
    println!("cargo::rustc-cfg=nibkit_{system}");
}
