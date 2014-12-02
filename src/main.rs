#![feature(macro_rules, phase)]
#![feature(phase)]
extern crate regex;
#[phase(plugin)] extern crate regex_macros;
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

use std::error::Error;
use std::io::net::ip::Ipv4Addr;
use std::io::ByRefWriter;

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

fn do_404(mut res: Response) {
    *res.status_mut() = hyper::status::NotFound;
    try_return!(res.start().and_then(|res| res.end()));
}

fn get_src_url(path: &str) -> Option<Url>{
    let mut full_url = String::from_str("http://localhost");
    full_url.push_str(path);
    println!("Full url is {}", full_url.as_slice());
    match Url::parse(full_url.as_slice()) {
        Ok(url) => {
            match url.query_pairs() {
                Some(pairs) => {
                    for &(ref key, ref val) in pairs.iter() {
                        if key.as_slice() == "src" {
                            println!("Found src: {}", val.as_slice());
                            match Url::parse(val.as_slice()) {
                                Ok(src_url) => return Some(src_url),
                                Err(e) => { println!("Error parsing src url: {}", e); return None; }
                            }
                        }
                    }
                    println!("No src query param");
                    None
                },
                None => { println!("No query params in url"); None }
            }
        },
        Err(e) => { println!("Error parsing url: {}", e); None }
    }
}

#[allow(unused_mut)]
fn parse_request(mut req: Request, mut res: Response) {
    // 404 on wrongly formatted requests
    if req.method != Get {
        do_404(res);
        return;
    }
    let path = match req.uri {
        hyper::uri::AbsolutePath(ref path) => path.as_slice(),
        _ => { do_404(res); return; }
    };
    println!("Incoming path: {}", path);

    // Respond to health ELB check
    if path == "/health-check" {
        *res.status_mut() = hyper::status::Ok;
        try_return!(res.start().and_then(|res| res.end()));
        println!("Health check");
        return;
    }

    // Check for transcode requests
    let re = regex!("^/(?P<format>png|jpg)/(?P<width>[0-9]{2,4})/(?P<height>[0-9]{2,4})/");
    if !re.is_match(path) {
        do_404(res);
        return;
    }
    println!("Path is a match. Determining parameters...")

    // use regex to disect incoming path OR 404
    let caps = re.captures(path).unwrap();
    let format = match caps.name("format") {
        "png" => image::PNG,
        "jpg" => image::JPEG,
        _ => unreachable!()
    };
    let width: u32 = match from_str(caps.name("width")) {
        Some(width) => width,
        None => { println!("Invalid width {}", caps.name("width")); do_404(res); return; }
    };
    let height: u32 = match from_str(caps.name("height")) {
        Some(height) => height,
        None => { println!("Invalid width {}", caps.name("height")); do_404(res); return; }
    };
    let url = match get_src_url(path) {
        Some(url) => url,
        None => { do_404(res); return; }
    };
    println!("Format {} | Width {}px | Height {}px ", format, width, height);

    // Transcode
    let mut img_in = match load_img_from_url(url) {
        Ok(img) => { println!("Successfully loaded image into memory"); img },
        Err(e) => { println!("Error loading img from url {}", e); return; }
    };
    let img_out = img_in.resize_exact(width, height, image::FilterType::Nearest);

    // Send back dummy response
    match format {
        image::JPEG => res.headers_mut().set(ContentType(Mime(Image,Jpeg,vec!()))),
        image::PNG => res.headers_mut().set(ContentType(Mime(Image,Png,vec!()))),
        _ => unreachable!()
    };
    let mut res = try_return!(res.start());
    {
        img_out.save(res.by_ref(), format);
        println!("Sent back image");
    }
    try_return!(res.end());
}

fn main() {
    let server = Server::http(Ipv4Addr(0, 0, 0, 0), 1337);
    server.listen(parse_request).unwrap();
    println!("Listening on http://0.0.0.0:1337");
}
