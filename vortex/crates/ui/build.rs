//! Build script for compiling Slint UI files.

#![allow(clippy::expect_used)]

fn main() {
    slint_build::compile("src/ui/main_window.slint").expect("Slint compilation failed");
}
