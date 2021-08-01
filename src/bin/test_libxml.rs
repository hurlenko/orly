use libxml::parser::Parser;
use libxml::tree::SaveOptions;
use libxml::xpath::Context;
use std::io::{self, Write};

fn main() {
    println!("here0");
    let parser = Parser::default();
    let doc = parser.parse_file("test.html").expect("here1");
    let opts = SaveOptions {
        as_xml: true,
        ..Default::default()
    };

    let doc_str = doc.to_string_with_options(opts);
    // println!("data: {}", &doc_str);
    io::stdout().write_all(doc_str.as_bytes()).expect("failed to write");
    // let context = Context::new(&doc).unwrap();
    // let result = context.evaluate("//p/@title").unwrap();

    // for node in &result.get_nodes_as_vec() {
    //   println!("Found: {}", node.get_content());
    // }
}
