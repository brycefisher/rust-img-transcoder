extern crate hyper;
extern crate mime;
extern crate image;

use hyper::Url;
use hyper::client::Request;
use hyper::header::common::ContentType;
use mime::{Mime, Image, Jpeg, Png, Gif};
use image::{load_from_memory, GenericImage};
use std::io::File;
use std::rand;

fn main() {
    let url = match Url::parse("http://c2.staticflickr.com/8/7384/12315308103_94b0a3f6cd_c.jpg") {
        Ok(url) => {
            println!("GET {}...", url)
            url
        },
        Err(e) => panic!("Invalid URL: {}", e)
    };

    let req = match Request::get(url) {
        Ok(req) => req,
        Err(err) => panic!("Failed to connect: {}", err)
    };

    let mut res = req
        .start().unwrap() // failure: Error writing Headers
        .send().unwrap(); // failure: Error reading Response head.

    println!("Response: {}", res.status);

    let format = match res.headers.get::<ContentType>() {
        Some(&ContentType(Mime(Image, Png, _))) => image::PNG,
        Some(&ContentType(Mime(Image, Jpeg, _))) => image::JPEG,
        Some(&ContentType(Mime(Image, Gif, _))) => image::GIF,
        Some(&ContentType(ref m)) => panic!("Content type {} not supported", m),
        None => panic!("No content type provided by remote server")
    };

    println!("{}", format);

    let body = match res.read_to_end() {
        Ok(v) => v.clone(),
        Err(e) => panic!("Unable to read http response body: {}", e)
    };
    let img_in = match load_from_memory(body.as_slice_(), format) {
        Ok(img) => img,
        Err(e) => panic!("Error reading image: {}", e)
    };

    let fout = File::create(&Path::new("test.png")).unwrap();
    let _ = img_in.save(fout, image::PNG);
}
