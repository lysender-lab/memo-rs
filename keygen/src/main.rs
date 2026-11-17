use cipher::encryption;

fn main() {
    println!("Generating a random master key...");
    let master_key = encryption::generate_key();
    println!("Master key: {}", master_key);
}
