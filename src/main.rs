extern crate hyper;
extern crate mime;
extern crate image;

use hyper::Url;
use hyper::client::Request;
use hyper::header::common::ContentType;
use mime::{Mime, Image, Jpeg, Png, Gif};
use image::{load_from_memory, GenericImage, DynamicImage};
use std::io::File;
use std::error::Error;
use TranscodeError::{HttpStatusError, UnsupportedContentType, HttpBodyError, PistonImageLoadError, RemoteServerUnreachable};

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
    let req = match Request::get(url) {
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

fn main() {
    let url = match Url::parse("http://c2.staticflickr.com/8/7384/12315308103_94b0a3f6cd_c.jpg") {
        Ok(url) => {
            println!("GET {}...", url)
            url
        },
        Err(e) => panic!("Invalid URL: {}", e)
    };

    let img_in = match load_img_from_url(url) {
        Ok(img) => img,
        Err(e) => panic!("Error loading img from url {}", e)
    };

    let fout = File::create(&Path::new("test.png")).unwrap();
    let _ = img_in.save(fout, image::PNG);
}
