pub struct Logger;

impl Logger {
    pub fn info(msg: &str) {
        println!("[INFO] {}", msg);
    }

    pub fn error(msg: &str) {
        eprintln!("[ERROR] {}", msg);
    }

    pub fn warn(msg: &str) {
        eprintln!("[WARN] {}", msg);
    }

    pub fn debug(msg: &str) {
        #[cfg(debug_assertions)]
        println!("[DEBUG] {}", msg);
    }
}

