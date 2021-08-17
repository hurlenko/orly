use libxml::{
    bindings::{xmlBufferContent, xmlBufferCreate, xmlBufferFree, xmlNodeDump, xmlNodePtr},
    readonly::RoNode,
    tree::{Document, Node, SaveOptions},
    xpath::{Context as XpathContext, Object},
};
use std::{ffi::CStr, os::raw::c_char};

pub(crate) trait NodeType {
    fn node_ptr(&self) -> xmlNodePtr;
}

impl NodeType for RoNode {
    fn node_ptr(&self) -> xmlNodePtr {
        self.node_ptr()
    }
}

impl NodeType for Node {
    fn node_ptr(&self) -> xmlNodePtr {
        self.node_ptr()
    }
}

pub(crate) trait DocumentExt {
    fn evaluate_xpath(&self, query: &str) -> Option<Object>;
    fn node_to_string_with_options<T: NodeType>(&self, node: &T, options: SaveOptions) -> String;

    fn node_to_string<T: NodeType>(&self, node: &T) -> String {
        self.node_to_string_with_options(node, Default::default())
    }

    fn xpath(&self, query: &str) -> Vec<RoNode> {
        match self.evaluate_xpath(query) {
            Some(object) => object.get_readonly_nodes_as_vec(),
            None => Vec::new(),
        }
    }
    fn xpath_mut(&self, query: &str) -> Vec<Node> {
        match self.evaluate_xpath(query) {
            Some(object) => object.get_nodes_as_vec(),
            None => Vec::new(),
        }
    }

    fn iterlinks(&self) -> Vec<(Node, Vec<String>)> {
        let link_attrs = [
            "action",
            "archive",
            "background",
            "cite",
            "classid",
            "codebase",
            "data",
            "href",
            "longdesc",
            "profile",
            "src",
            "usemap",
            // Not standard:
            "dynsrc",
            "lowsrc",
            // HTML5 formaction
            "formaction",
        ];

        let query = format!(
            "//*[{}]",
            link_attrs
                .iter()
                .map(|&attr| format!("@{}", attr))
                .collect::<Vec<String>>()
                .join(" or ")
        );

        self.xpath_mut(&query)
            .into_iter()
            .map(|node| {
                let found = link_attrs
                    .iter()
                    .filter_map(|attr| {
                        node.get_attribute(attr).map(|_| attr.to_string())
                    })
                    .collect();
                (node, found)
            })
            .collect()
    }

    fn rewrite_links<F: Fn(&str) -> String>(&self, link_repl_func: F) {
        for (mut node, attrs) in self.iterlinks() {
            for attr in attrs {
                if let Some(curr_url) = node.get_attribute(&attr) {
                    let new_url = link_repl_func(&curr_url);
                    // if new_url != curr_url {
                    //     println!("Old: {}\nNew: {}\n", curr_url, new_url);
                    // }
                    if node.set_attribute(&attr, &new_url).is_err() {
                        println!("Failed to set node attr {}", attr);
                    }
                }
            }
        }
    }
}

pub(crate) trait SaveOptionsExt {
    fn as_mask(&self) -> i32;
}

impl SaveOptionsExt for SaveOptions {
    fn as_mask(&self) -> i32 {
        let params = [
            self.format,
            self.no_declaration,
            self.no_empty_tags,
            self.no_xhtml,
            self.xhtml,
            self.as_xml,
            self.as_html,
            self.non_significant_whitespace,
        ];

        params
            .iter()
            .enumerate()
            .map(|(idx, &flag)| if flag { 1 << idx } else { 0 })
            .sum()
    }
}

impl DocumentExt for Document {
    fn evaluate_xpath(&self, query: &str) -> Option<Object> {
        let context: XpathContext = XpathContext::new(self).ok()?;
        context.evaluate(query).ok()
    }

    fn node_to_string_with_options<T: NodeType>(&self, node: &T, options: SaveOptions) -> String {
        unsafe {
            // allocate a buffer to dump into
            let buf = xmlBufferCreate();

            // dump the node
            xmlNodeDump(buf, self.doc_ptr(), node.node_ptr(), 1, options.as_mask());
            let result = xmlBufferContent(buf);
            let c_string = CStr::from_ptr(result as *const c_char);
            let node_string = c_string.to_string_lossy().into_owned();
            xmlBufferFree(buf);

            node_string
        }
    }
}
