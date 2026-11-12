//! Recompile la crate lorsqu'une migration est ajoutée ou modifiée
//! (les migrations sont embarquées par `sqlx::migrate!`).

fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
