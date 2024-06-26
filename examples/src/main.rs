use em_bindgen::future::block_on;
use http::Method;
use std::time::SystemTime;

pub fn main() {
    block_on(async move {
        let response = em_bindgen::fetch::Builder::new(
            Method::GET,
            c"https://pokeapi.co/api/v2/pokemon/ditto",
        )
        .send()
        .await
        .unwrap();

        println!("{response:?}");
    });

    // set_main_loop(main_loop, Some(Timing::from(Duration::from_secs(1))), true)
}

pub fn main_loop() {
    println!("Hello from {:?}", SystemTime::now())
}
