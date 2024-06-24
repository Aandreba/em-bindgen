use std::time::{Duration, SystemTime};

use em_bindgen::{set_main_loop, Timing};

#[no_mangle]
pub extern "C" fn main() {
    set_main_loop(main_loop, Some(Timing::from(Duration::from_secs(1))), true)
}

pub fn main_loop() {
    println!("Hello from {:?}", SystemTime::now())
}
