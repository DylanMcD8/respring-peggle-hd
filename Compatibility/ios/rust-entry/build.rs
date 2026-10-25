fn main() {
    // Rust's staticlib includes SDL's objects. Relink when the local SDL fork
    // changes rather than keeping a cached archive with the previous objects.
    println!("cargo:rerun-if-env-changed=CHUZZLE_SDL_LIBRARY");
    if let Ok(library) = std::env::var("CHUZZLE_SDL_LIBRARY") {
        println!("cargo:rerun-if-changed={library}");
    }
}
