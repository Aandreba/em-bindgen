use em_bindgen::future::block_on;
use std::time::{Duration, SystemTime};

pub fn main() {
    block_on(async move {
        em_bindgen::future::spawn_local(async move {
            println!("Hi from task!");
            em_bindgen::future::sleep(Duration::from_secs(1)).await;
            println!("1 second has passed");
        });

        println!("Hello");
        em_bindgen::future::sleep(Duration::from_secs(2)).await;
        println!("World!");
    });

    // set_main_loop(main_loop, Some(Timing::from(Duration::from_secs(1))), true)
}

pub fn main_loop() {
    println!("Hello from {:?}", SystemTime::now())
}
