// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This crate provides CORS support for [Rocket](https://rocket.rs)
//! by implementing a fairing.
//!
//! # Example:
//! ```
//! # #[macro_use] extern crate rocket_cors;
//! # #[macro_use] extern crate rocket;
//! # fn main() {
//! use rocket::http::Method;
//! use rocket_cors::CORS;
//!
//! let cors = cors!("/api/:user/action" => Method::Get, Method::Put;
//!                  "/api/:user/delete" => Method::Delete);
//! let cors2 = cors!("/api/:user/add" => Method::Post);
//! let rocket = rocket::ignite().attach(cors).attach(cors2);
//!
//! # }
//! ```
#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate hyper;
extern crate rocket;
extern crate unicase;

use hyper::header::{AccessControlAllowHeaders, AccessControlAllowMethods, AccessControlAllowOrigin};
use hyper::method::Method::{Delete, Get, Post, Put};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Method, Status};
use rocket::{Request, Response};
use rocket::response::Body;
use std::io::Cursor;
use unicase::UniCase;

/// A tuple binding together a set of HTTP methods and a url path.
pub type CORSEndpoint = (Vec<Method>, String);

/// Helper macro to build a vector of `CORSEndpoint` value(s).
#[macro_export]
macro_rules! cors {
    ($($path:expr => $($method:expr),+);+) => (
        CORS::new(vec![$((vec![$($method),+], $path.to_owned())),+])
    )
}

pub struct CORS {
    allowed_endpoints: Vec<CORSEndpoint>,
}

impl CORS {
    /// Creates a new CORS fairing from a vector of `CORSEndpoint`.
    /// Only endpoints listed here will allow CORS.
    /// Endpoints containing a variable path part can use ':foo' like in:
    /// '/foo/:bar' for a URL like https://domain.com/foo/123 where 123 is
    /// variable.
    pub fn new(endpoints: Vec<CORSEndpoint>) -> Self {
        CORS {
            allowed_endpoints: endpoints,
        }
    }

    fn is_allowed(&self, request: &Request) -> bool {
        let mut is_cors_endpoint = false;
        for endpoint in self.allowed_endpoints.clone() {
            let (methods, path) = endpoint;

            if !methods.contains(&request.method()) && request.method() != Method::Options {
                continue;
            }

            let path: Vec<&str> = if path.starts_with('/') {
                path[1..].split('/').collect()
            } else {
                path[0..].split('/').collect()
            };

            let uri: Vec<&str> = request.uri().segments().collect();

            if path.len() != uri.len() {
                continue;
            }

            for i in 0..uri.len() {
                is_cors_endpoint = false;
                if uri[i] != path[i] && !path[i].starts_with(':') {
                    break;
                }
                is_cors_endpoint = true;
            }
            if is_cors_endpoint {
                break;
            }
        }
        is_cors_endpoint
    }

    fn add_headers(response: &mut Response) {
        response.set_header(AccessControlAllowOrigin::Any);
        response.set_header(AccessControlAllowHeaders(vec![
            UniCase(String::from("accept")),
            UniCase(String::from("accept-language")),
            UniCase(String::from("authorization")),
            UniCase(String::from("content-type")),
        ]));
        response.set_header(AccessControlAllowMethods(vec![Get, Post, Put, Delete]));
    }
}

impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "CORS support",
            kind: Kind::Response,
        }
    }

    fn on_response(&self, request: &Request, mut response: &mut Response) {
        if self.is_allowed(request) {
            CORS::add_headers(&mut response);
            if request.method() == Method::Options {
                // Just return an empty response for CORS Options.
                response.set_status(Status::Ok);
                response.set_raw_body(Body::Sized(Cursor::new(""), 0));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::CORS;
    use rocket::{self, Response};
    use rocket::http::{Method, Status};
    use rocket::local::Client;

    #[get("/endpoint")]
    fn endpoint() -> &'static str {
        "Hello World!"
    }

    fn verify_no_cors_reponse(response: &mut Response) {
        assert_eq!(response.status(), Status::Ok);

        let body_str = response.body().and_then(|b| b.into_string());
        assert_eq!(body_str, Some("Hello World!".to_string()));

        let values: Vec<_> = response
            .headers()
            .get("Access-Control-Allow-Origin")
            .collect();
        assert_eq!(values.len(), 0);
    }

    fn verify_cors_response_with(response: &mut Response, body: &str) {
        assert_eq!(response.status(), Status::Ok);

        let body_str = response.body().and_then(|b| b.into_string());
        assert_eq!(body_str, Some(body.to_string()));

        let values: Vec<_> = response
            .headers()
            .get("Access-Control-Allow-Origin")
            .collect();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "*");

        let values: Vec<_> = response
            .headers()
            .get("Access-Control-Allow-Headers")
            .collect();
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0],
            "accept, accept-language, authorization, content-type"
        );

        let values: Vec<_> = response
            .headers()
            .get("Access-Control-Allow-Methods")
            .collect();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "GET, POST, PUT, DELETE");
    }

    fn verify_cors_response(response: &mut Response) {
        verify_cors_response_with(response, "Hello World!")
    }

    #[test]
    fn no_cors() {
        let rocket = rocket::ignite().mount("/", routes![endpoint]);
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/endpoint").dispatch();
        verify_no_cors_reponse(&mut response);
    }

    #[test]
    fn cors_simple() {
        let rocket = rocket::ignite()
            .mount("/", routes![endpoint])
            .attach(cors!("/endpoint" => Method::Get, Method::Put));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/endpoint").dispatch();
        verify_cors_response(&mut response);
    }

    #[test]
    fn cors_simple_no_slash() {
        let rocket = rocket::ignite()
            .mount("/", routes![endpoint])
            .attach(cors!("/endpoint" => Method::Get));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/endpoint").dispatch();
        verify_cors_response(&mut response);
    }

    #[test]
    fn cors_bad_method() {
        let rocket = rocket::ignite()
            .mount("/", routes![endpoint])
            .attach(cors!("/endpoint" => Method::Put));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/endpoint").dispatch();
        verify_no_cors_reponse(&mut response);
    }

    #[test]
    fn cors_wrong_path_len() {
        let rocket = rocket::ignite()
            .mount("/", routes![endpoint])
            .attach(cors!("/some/endpoint" => Method::Get));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/endpoint").dispatch();
        verify_no_cors_reponse(&mut response);
    }

    #[test]
    fn cors_wrong_path_segments() {
        let rocket = rocket::ignite()
            .mount("/another", routes![endpoint])
            .attach(cors!("/some/endpoint" => Method::Get));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/another/endpoint").dispatch();
        verify_no_cors_reponse(&mut response);
    }

    #[test]
    fn cors_test_preflight() {
        let rocket = rocket::ignite()
            .mount("/", routes![endpoint])
            .attach(cors!("/endpoint" => Method::Get));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.options("/endpoint").dispatch();
        verify_cors_response_with(&mut response, "");
    }

    #[test]
    fn cors_variable_path() {
        let rocket = rocket::ignite()
            .mount("/cors", routes![endpoint])
            .attach(cors!("/cors/:something" => Method::Get));
        let client = Client::new(rocket).expect("valid rocket instance");
        let mut response = client.get("/cors/endpoint").dispatch();

        verify_cors_response(&mut response);
    }
}
