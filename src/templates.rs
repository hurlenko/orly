use askama::Template; // bring trait in scope

#[derive(Template)]
#[template(path = "base.html")]
pub struct BaseHtml<'a> {
    pub styles: &'a str,
    pub body: &'a str,
    pub should_support_kindle: bool,
}

#[derive(Template)]
#[template(path = "container.xml")]
pub struct ContainerXml;
