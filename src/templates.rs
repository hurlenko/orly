use askama::Template;

#[derive(Template)]
#[template(path = "base.html")]
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
