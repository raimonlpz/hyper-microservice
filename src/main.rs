// CLI (hot reload) -> cargo watch -x "run"
use futures::{future, Future};
use hyper::service::service_fn;
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use regex::Regex;
use slab::Slab;
use std::fmt;
use std::sync::{Arc, Mutex};

fn main() {
    // Socket address that consists of an IP address and a port number (IPv4).
    // Using the SocketAddr struct, which contains both the IpAddr and the u16 from the tuple ([u8; 4], 16).
    // RUST TRAIT -> impl<I: Into<IpAddr>> From<(I, u16)> for SocketAddr --> impl From<[u8; 4]> for IpAddr
    // .into() method call to construct a socket address from the tuple
    let addr = ([127, 0, 0, 1], 8080).into();

    // We create a server instance and bind it to this address, it actually returns Builder, not a Server instance.
    let builder = Server::bind(&addr);

    // We also have to send the reference (of this shared state) to the main func
    let user_db = Arc::new(Mutex::new(Slab::new()));

    // The Builder struct provides methods to tweak the parameters of the server created
    // We use builder to attach a service for handling incoming HTTP requests using the serve method
    // Ultimately it generates a Service instance
    // The generated item then has to implement the hyper::service::Service trait.
    // A service in a hyper crate is a function that takes a request and gives a response back.
    // Instead, we'll use the service_fn_ok function, which turns a function with suitable types into a service handler
    // There are two corresponding structs: hyper::Request and hyper::Response.

    /* let server = builder.serve(|| {
        service_fn_ok(|_| {
            Response::new(Body::from(
                "<h1><i>Almost</i> microservice...</h1>
            <ul>
            <li>Hey</li>
            <li>Ho</li>
            </ul>
        ",
            ))
        })
    }); */

    // When the reference moves into the closure, we can send it to microservice_handler.
    // This is a handler function called by a closure sent to the service_fn call.
    // We have to clone the reference to move it to a nested closure, because that closure can be called multiple times.
    let server = builder.serve(move || {
        let user_db = Arc::clone(&user_db);
        service_fn(move |req| microservice_handler(req, &user_db))
    });

    // The runtime expects a Future instance with the Future<Item = (), Error = ()> type
    // In our example we'll just drop any error
    let server = server.map_err(drop);

    // We can start the server with the specific runtime
    hyper::rt::run(server);
}

// This function expects a Request, but it doesn't return a simple Response instance. Instead, it returns a future result.
// We can return an implementation of the trait by value, rather than by reference. Our future can be resolved to a
// hyper::Response<Body> item or a hyper::Error error type.

// Our service function will support three kinds of requests:
// - GET requests to the '/' path with an index page response |
// - Actions with user data (prefix/user/) |
// - Other requests with a NOT_FOUND response |
// To detect the corresponding method and path, we can use the methods of the Request object

// To get access to a shared state, you need to provide a reference to the shared data.
// Now our service func has an extra argument, which is the reference to the shared state:
fn microservice_handler(
    req: Request<Body>,
    user_db: &UserDb,
) -> impl Future<Item = Response<Body>, Error = Error> {
    let response = {
        let method = req.method();
        let path = req.uri().path();
        let mut users = user_db.lock().unwrap();

        if INDEX_PATH.is_match(path) {
            if method == &Method::GET {
                Response::new(INDEX.into())
            } else {
                response_with_code(StatusCode::METHOD_NOT_ALLOWED)
            }
        } else if USERS_PATH.is_match(path) {
            if method == &Method::GET {
                let list = users
                    .iter()
                    .map(|(id, _)| id.to_string())
                    .collect::<Vec<String>>()
                    .join(",");
                Response::new(list.into())
            } else {
                response_with_code(StatusCode::METHOD_NOT_ALLOWED)
            }
        } else if let Some(cap) = USER_PATH.captures(path) {
            let user_id = cap
                .name("user_id")
                .and_then(|m| m.as_str().parse::<UserId>().ok().map(|x| x as usize));
            match (method, user_id) {
                (&Method::GET, Some(id)) => {
                    if let Some(data) = users.get(id) {
                        Response::new(Body::from(data.to_string()))
                    } else {
                        response_with_code(StatusCode::NOT_FOUND)
                    }
                }

                (&Method::PUT, Some(id)) => {
                    if let Some(user) = users.get_mut(id) {
                        *user = UserData;
                        response_with_code(StatusCode::OK)
                    } else {
                        response_with_code(StatusCode::NOT_FOUND)
                    }
                }

                (&Method::POST, None) => {
                    let id = users.insert(UserData);
                    Response::new(Body::from(id.to_string()))
                }

                (&Method::POST, Some(_)) => response_with_code(StatusCode::BAD_REQUEST),

                (&Method::DELETE, Some(id)) => {
                    if users.contains(id) {
                        users.remove(id);
                        response_with_code(StatusCode::OK)
                    } else {
                        response_with_code(StatusCode::NOT_FOUND)
                    }
                }
                _ => response_with_code(StatusCode::METHOD_NOT_ALLOWED),
            }
        } else {
            response_with_code(StatusCode::NOT_FOUND)
        }

        // match (req.method(), req.uri().path()) {
        //     (&Method::GET, "/") => Response::new(Body::from(INDEX)),
        //     // we use an if expression to detect that the path starts with '/user/' prefix
        //     (method, path) if path.starts_with(USER_PATH) => {
        //         // the str::trim_left_matches method removes the part of the string if it matches a provided string from the arg
        //         // we use the str::parse method, which tries to convert a string (the remaining tail) to a type that implements the FromStr trait of the standard library.
        //         // UserId already implements this, because it's equal to the u64 type, which can be parsed from the string.
        //         // The parse method returns Result. We convert this to an Option instance with Result::ok functions.
        //         let user_id = path
        //             .trim_start_matches(USER_PATH)
        //             .parse::<UserId>()
        //             .ok()
        //             .map(|x| x as usize);
        //         let mut users = user_db.lock().unwrap();

        // match (method, user_id) {
        //     // When the data is created, we need to be able to read it.
        //     (&Method::GET, Some(id)) => {
        //         if let Some(data) = users.get(id) {
        //             Response::new(data.to_string().into())
        //         } else {
        //             response_with_code(StatusCode::NOT_FOUND)
        //         }
        //     }

        //     // Once the data is saved, we might want to provide the ability to modify it.
        //     (&Method::PUT, Some(id)) => {
        //         // Code tries to find a user instance in the user database with the get_mut method.
        //         // This returns a mutable reference wrapped with either a Some option, or a None option.
        //         // We can use a dereference operator, *, to replace the data in the storage.
        //         if let Some(user) = users.get_mut(id) {
        //             *user = UserData;
        //             response_with_code(StatusCode::OK)
        //         } else {
        //             response_with_code(StatusCode::NOT_FOUND)
        //         }
        //     }

        //     // When the server has just started, it doesn't contain any data. To support data creation, we use the POST method without the user's ID.
        //     // This code adds a UserData instance to the user database and sends the associated ID of the user in a response with the OK status (an HTTP status code of 200).
        //     (&Method::POST, None) => {
        //         let id = users.insert(UserData);
        //         Response::new(Body::from(id.to_string()))
        //     }
        //     // What if the client sets the ID with a POST request? We'll inform the client that the request was wrong.
        //     (&Method::POST, Some(_)) => response_with_code(StatusCode::BAD_REQUEST),

        //     // When we don't need data anymore, we can delete it.
        //     (&Method::DELETE, Some(id)) => {
        //         if users.contains(id) {
        //             users.remove(id);
        //             response_with_code(StatusCode::OK)
        //         } else {
        //             response_with_code(StatusCode::NOT_FOUND)
        //         }
        //     }

        //     _ => response_with_code(StatusCode::METHOD_NOT_ALLOWED),
        // }
        //     }
        //     _ => response_with_code(StatusCode::NOT_FOUND),
        // }
    };
    future::ok(response)
}

lazy_static! {
    // index.htm | index.html | /
    static ref INDEX_PATH: Regex = Regex::new("^/(index\\.html?)?$").unwrap();
    // /user/ | /user/<id> | /user/<id>/
    static ref USER_PATH: Regex = Regex::new("^/user/((?P<user_id>\\d+?)/?)?$").unwrap();
    // /users/ | /users
    static ref USERS_PATH: Regex = Regex::new("^/users/?$").unwrap();
}
//const USER_PATH: &str = "/user/";

// HTML code. r#...# is for multiline string blobs
const INDEX: &'static str = r#"
 <!doctype html>
 <html>
     <head>
         <title>Rust Microservice</title>
     </head>
     <body>
         <h3>Rust Microservice</h3>
     </body>
 </html>
 "#;

// Some types to handle a user database, which will hold data about users
type UserId = u64;
struct UserData;
// Arc is an atomic reference counter that provides multiple references to a single instance of data.
// Atomic entities can be safely used with multiple threads.
// Mutex is a mutual-exclusion wrapper that controls access to mutable data. Mutex is an atomic flag that
// checks that only one thread has access to the data, and other threads have to wait until the thread that
// has locked the mutex releases it.
// Slab is an allocator that can store and remove any value identified by an ordered number
// In this case, we use Slab to allocate new IDs for users and to keep the data with the user.
// We use Arc with the Mutex pair to protect our database of data race, because different responses can be processed in different threads, which can both try to access the database.
type UserDb = Arc<Mutex<Slab<UserData>>>;

// We need a helper function that creates empty responses with the corresponding HTTP status codes
// This func expects a status code, creates a new response builder, sets the status and adds an empty body
fn response_with_code(status_code: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .body(Body::empty())
        .unwrap()
}

// To make the UserData convertible to a String, we have to implement the ToString trait for that type.
// However, it's typically more useful to implement the Display trait
// In this implementation, we return a string with an empty JSON object "{}".
impl fmt::Display for UserData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("{}")
    }
}
