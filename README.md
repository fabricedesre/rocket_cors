# A CORS fairing for Rocket

This crate provides CORS support for [Rocket](https://rocket.rs) by implementing a fairing.

## Example

```rust
#[macro_use] extern crate rocket_cors;
#[macro_use] extern crate rocket;
fn main() {
    use rocket::http::Method;
    use rocket_cors::CORS;

    let cors = cors!("/api/:user/action" => Method::Get, Method::Put;
                    "/api/:user/delete" => Method::Delete);
    let cors2 = cors!("/api/:user/add" => Method::Post);
    let rocket = rocket::ignite().attach(cors).attach(cors2);
}
```
