/// Client binary entry point — delegates to the shared library crate.
/// All game logic lives in `src/lib.rs`; this thin wrapper exists so that
/// the desktop and WASM builds have a standard `fn main()` entry point while
/// the library crate provides the Android `android_main` entry point via
/// the `#[bevy_main]` attribute.
fn main() {
    rustshootergame::main();
}
