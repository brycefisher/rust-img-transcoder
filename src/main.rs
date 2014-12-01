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

fn do_404(mut res: Response) {
    *res.status_mut() = hyper::status::NotFound;
    try_return!(res.start().and_then(|res| res.end()));
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

    // use regex to disect incoming path OR 404
    println!("Incoming path: {}", path);
    let re = regex!("^/(?P<format>png|jpg)/(?P<width>[0-9]{2,4})/(?P<height>[0-9]{2,4})/");
    if !re.is_match(path) {
        do_404(res);
        return;
    }

    println!("Path is a match. Determining parameters...")
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
    println!("Format {} | Width {}px | Height {}px", format, width, height);

    // Transcode hardcoded path
    let url = Url::parse("http://c2.staticflickr.com/8/7384/12315308103_94b0a3f6cd_c.jpg").unwrap();
    let mut img_in = match load_img_from_url(url) {
        Ok(img) => { println!("Successfully loaded image into memory"); img },
        Err(e) => { println!("Error loading img from url {}", e); return; }
    };

    let fout = File::create(&Path::new("test.png")).unwrap();
    let _ = img_in
        .resize_exact(width, height, image::FilterType::Nearest)
        .save(fout, format);
    println!("Saved image to disk as a {}", format);

    // Send back dummy response
    let out = b"Saved image to disk";
    res.headers_mut().set(ContentLength(out.len()));
    let mut res = try_return!(res.start());
    try_return!(res.write(out));
    try_return!(res.end());
}

fn main() {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
    server.listen(parse_request).unwrap();
    println!("Listening on http://127.0.0.1:1337");
}
