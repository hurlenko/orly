
use std::default::Default;
use std::io::{self, Write};

use html5ever::driver::ParseOpts;
use html5ever::tree_builder::TreeBuilderOpts;
use xml5ever::tendril::TendrilSink;
use xml5ever::driver::parse_document;
use xml5ever::serialize::serialize as xmlserialize;
use markup5ever_rcdom::{RcDom, SerializableHandle};

fn main() {
    let html_opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let stdin = io::stdin();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut stdin.lock())
        .unwrap();

    // The validator.nu HTML2HTML always prints a doctype at the very beginning.
    io::stdout()
        .write_all(b"<!DOCTYPE html>\n")
        .ok()
        .expect("writing DOCTYPE failed");
    let document: SerializableHandle = dom.document.clone().into();
    xmlserialize(&mut io::stdout(), &document, Default::default())
        .ok()
        .expect("serialization failed");
}