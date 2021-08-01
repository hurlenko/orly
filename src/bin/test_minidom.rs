use minidom::Element;
use std::fs::File;
use std::io::prelude::*;

fn main() {
    let root: Element = include_str!("../../test.html").parse().unwrap();
    let mut buffer = File::create("minnidom.xml").unwrap();
    root.write_to(&mut buffer).unwrap();

    // let mut articles: Vec<Article> = Vec::new();

    // for child in root.children() {
    //     if child.is("article", ARTICLE_NS) {
    //         let title = child.get_child("title", ARTICLE_NS).unwrap().text();
    //         let body = child.get_child("body", ARTICLE_NS).unwrap().text();
    //         articles.push(Article {
    //             title: title,
    //             body: body.trim().to_owned(),
    //         });
    //     }
    // }

    // println!("{:?}", root);
}
