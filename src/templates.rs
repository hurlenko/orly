use askama::Template;

#[derive(Template)]
#[template(path = "base.xhtml", escape = "xml")]
pub struct BaseHtml<'a> {
    pub styles: &'a Vec<&'a String>,
    pub body: &'a str,
    pub should_support_kindle: bool,
}

#[derive(Template)]
#[template(path = "container.xml")]
pub struct ContainerXml;

#[derive(Template)]
#[template(path = "ibooks.xml")]
pub struct IbooksXml;

#[derive(Template)]
#[template(path = "navpoint.xml")]
pub struct NavPoint<'a> {
    pub id: &'a str,
    pub order: usize,
    pub label: &'a str,
    pub url: &'a str,
    pub children: Vec<NavPoint<'a>>,
}

#[derive(Template)]
#[template(path = "toc.xml")]
pub struct Toc<'a> {
    pub uid: &'a str,
    pub depth: usize,
    pub pagecount: usize,
    pub title: &'a str,
    pub author: &'a str,
    pub navpoints: &'a Vec<NavPoint<'a>>,
}
