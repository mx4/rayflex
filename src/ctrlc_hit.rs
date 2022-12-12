use std::sync::atomic::AtomicBool;

pub static CTRLC_HIT: AtomicBool = AtomicBool::new(false);
