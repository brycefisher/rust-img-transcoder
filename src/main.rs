#![feature(macro_rules, phase)]

extern crate hyper;
extern crate mime;
extern crate image;

use hyper::Url;
use hyper::client;
use hyper::header::common::{ContentType, ContentLength};
use hyper::Get;
use hyper::server::{Server, Request, Response};

use mime::{Mime, Image, Jpeg, Png, Gif};

#[allow(unused_imports)]
use image::{load_from_memory, GenericImage, DynamicImage};

use std::io::File;
use std::error::Error;
use std::io::net::ip::Ipv4Addr;

use TranscodeError::{HttpStatusError, UnsupportedContentType, HttpBodyError, PistonImageLoadError, RemoteServerUnreachable};

macro_rules! try_return(
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(e) => { println!("Error: {}", e); return; }
        }
    }}
)

#[deriving(Show, PartialEq, Clone)]
pub enum TranscodeError {
    HttpStatusError,
    UnsupportedContentType,
    HttpBodyError,
    PistonImageLoadError,
    RemoteServerUnreachable,
}

impl Error for TranscodeError {
    fn description(&self) -> &str {
        match *self {
            RemoteServerUnreachable => "Unable to connect with remote server",
            HttpStatusError => "Invalid HTTP status code",
            UnsupportedContentType => "Unsupported Content-Type header in HTTP response",
            HttpBodyError => "Error reading from the HTTP response body",
            PistonImageLoadError => "Image library was unable to load image into memory"
        }
    }
}

type TranscodeResult<T> = Result<T, TranscodeError>;

fn load_img_from_url(url: Url) -> TranscodeResult<DynamicImage> {
    let req = match client::Request::get(url) {
        Ok(req) => req,
        Err(_) => return Err(RemoteServerUnreachable)
    };

    let mut res = req
        .start().unwrap() // failure: Error writing Headers
        .send().unwrap(); // failure: Error reading Response head.

    let format = match res.headers.get::<ContentType>() {
        Some(&ContentType(Mime(Image, Png, _))) => image::PNG,
        Some(&ContentType(Mime(Image, Jpeg, _))) => image::JPEG,
        Some(&ContentType(Mime(Image, Gif, _))) => image::GIF,
        _ => return Err(UnsupportedContentType)
    };

    let body = match res.read_to_end() {
        Ok(v) => v.clone(),
        Err(_) => return Err(HttpBodyError)
    };

    match load_from_memory(body.as_slice_(), format) {
        Ok(img) => Ok(img),
        Err(_) => Err(PistonImageLoadError)
    }
}

#[allow(unused_mut)]
fn parse_request(mut req: Request, mut res: Response) {
    match req.uri {
        hyper::uri::AbsolutePath(ref path) => match (&req.method, path.as_slice()) {
            (&Get, "/") | (&Get, "/echo") => {
                let out = b"Try POST /echo";
                res.headers_mut().set(ContentLength(out.len()));
                let mut res = try_return!(res.start());
                try_return!(res.write(out));
                try_return!(res.end());
                return;
            },
            _ => {
                *res.status_mut() = hyper::status::NotFound;
                try_return!(res.start().and_then(|res| res.end()));
                return;
            }
        },
        _ => {
            try_return!(res.start().and_then(|res| res.end()));
            return;
        }
    };
}

fn main() {
    // Transcode
    //let url = match Url::parse("http://c2.staticflickr.com/8/7384/12315308103_94b0a3f6cd_c.jpg") {
    let url = match Url::parse("http://c2.staticflickr.com/6/5145/5548591309_b0c26f6b47_b.jpg") {
        Ok(url) => {
            println!("GET {}...", url)
            url
        },
        Err(e) => panic!("Invalid URL: {}", e)
    };

    let img_in = match load_img_from_url(url) {
        Ok(img) => { println!("Successfully loaded image into memory"); img },
        Err(e) => panic!("Error loading img from url {}", e)
    };

    let fout = File::create(&Path::new("test.png")).unwrap();
    let _ = img_in.save(fout, image::PNG);
    println!("Saved image to disk as a png");

    // Server
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
    server.listen(parse_request).unwrap();
    println!("Listening on http://127.0.0.1:1337");
}
