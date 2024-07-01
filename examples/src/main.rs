use em_bindgen::{future::block_on, set_main_loop};
use http::Method;
use std::time::{Duration, SystemTime};

pub fn main() {
    let mut count = 0;
    set_main_loop(
        move || {
            count += 1;
            println!("Count: {count}");
        },
        Some(em_bindgen::Timing::SetTimeout(Duration::from_millis(500))),
        true,
    )

    // set_main_loop(main_loop, Some(Timing::from(Duration::from_secs(1))), true)
}

pub async fn fetch_test() {
    let response =
        em_bindgen::fetch::Builder::new(Method::GET, c"https://pokeapi.co/api/v2/pokemon/ditto")
            .send()
            .await
            .unwrap();

    println!("{response:?}");
}
