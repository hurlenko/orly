use std::collections::HashSet;

use askama::Template;
use reqwest::Url; // bring trait in scope

#[derive(Template)]
#[template(path = "base.html")]
pub struct BaseHtml<'a> {
    pub styles: &'a HashSet<Url>,
    pub body: &'a str,
    pub should_support_kindle: bool,
}

#[derive(Template)]
#[template(path = "container.xml")]
pub struct ContainerXml;

#[derive(Template)]
#[template(path = "ibooks.xml")]
pub struct IbooksXml;
