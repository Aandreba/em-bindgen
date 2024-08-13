use em_bindgen::{
    cancel_main_loop, console::init_with_level, get_now, set_finite_main_loop,
    set_infinite_main_loop,
};
use http::Method;
use log::LevelFilter;
use std::time::Duration;

pub fn main() {
    init_with_level(LevelFilter::Debug).unwrap();

    let dropped_1 = Dropped(1);
    let dropped_2 = Dropped(2);

    let next_ms = get_now() + Duration::from_secs(2).as_millis() as f64;
    log::info!("{next_ms}");

    set_finite_main_loop(
        || {
            let dropped = &dropped_2;
            let now = get_now();
            log::info!("{now} v. {next_ms}");

            if now >= next_ms {
                cancel_main_loop();
            } else {
                log::info!("Hello!");
            }
        },
        None,
    );

    log::info!("I'm back!");
    drop(dropped_1);

    // println!("{:?}", FileDialog::default().load_file());
    // assert!(FileDialog::default()
    //     .set_file_name("hello.txt")
    //     .save_file(b"Hello world!"));
}

//* TESTS *//
pub async fn fetch_test() {
    let response =
        em_bindgen::fetch::Builder::new(Method::GET, c"https://pokeapi.co/api/v2/pokemon/ditto")
            .send()
            .await
            .unwrap();

    println!("{response:?}");
}

pub fn test_main_loop() {
    let mut count = 0;
    set_infinite_main_loop(
        move || {
            count += 1;
            println!("Count: {count}");
        },
        Some(em_bindgen::Timing::SetTimeout(Duration::from_millis(500))),
    )
}

struct Dropped(i32);

impl Drop for Dropped {
    fn drop(&mut self) {
        log::error!("I was dropped! {}", self.0);
    }
}
