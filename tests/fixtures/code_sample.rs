/// A sample Rust function for testing code extraction.
pub fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

/// A struct for testing struct extraction.
pub struct Config {
    pub name: String,
    pub value: i32,
}

impl Config {
    pub fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}
