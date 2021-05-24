use std::{fs::File, path::Path};

use http_req::{request, response::StatusCode};
use ordbog::{Dict, Mode};

fn get_wiki_words() -> Vec<String> {
    let url = "https://www.corpusdata.org/wiki/samples/text.zip";
    let zip_path = Path::new("text.zip");
    let txt_path_str = "text.txt";
    let txt_path = Path::new(txt_path_str);
    if !Path::exists(txt_path) {
        if !Path::exists(zip_path) {
            println!("downloading {}", url);
            let mut text_zip = File::create(zip_path).expect("writing zip");
            let req = request::get(url, &mut text_zip).expect("downloading zip");
            assert!(req.status_code() == StatusCode::new(200));
            assert!(Path::exists(zip_path));
        }
        println!("extracting {}", txt_path_str);
        let reader = File::open(zip_path).expect("reading zip");
        let mut zip = zip::ZipArchive::new(reader).expect("opening zip");
        let mut file = zip.by_name(txt_path_str).expect("accessing zip");
        let mut writer = File::create(txt_path).expect("writing txt");
        std::io::copy(&mut file, &mut writer).expect("extracting txt");
    }
    println!("loading words from {}", txt_path_str);
    std::fs::read_to_string(txt_path)
        .expect("reading txt")
        .split_ascii_whitespace()
        .map(String::from)
        .collect()
}

fn main() {
    let words = get_wiki_words();
    let dict = Dict::new(Mode::Byte, words);
    println!("produced dict with {} codes", dict.codes.len());
    for (i, val) in dict.codes.iter().enumerate() {
        println!("code 0x{:04x} = {:?}", 2 * (i + 1), val);
    }

    println!("querying dictionary");
    for word in vec!["", "and", "ape", "the", "thorn", "yolo", "zygote"] {
        println!(
            "query: {:?} => code 0x{:04x}",
            word,
            dict.encode(&String::from(word)).0
        );
    }
}
