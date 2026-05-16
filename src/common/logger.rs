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

    #[cfg(debug_assertions)]
    pub fn debug(msg: &str) {
        println!("[DEBUG] {}", msg);
    }

    #[cfg(not(debug_assertions))]
    pub fn debug(_msg: &str) {}
}
