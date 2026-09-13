use maudit::content::HeadingComponent;

const ANCHOR_ICON: &str =
    r#"<span class="docs-heading-symbol" aria-hidden="true">#</span>"#;

pub struct DocsHeading;

impl HeadingComponent for DocsHeading {
    fn render_start(&self, level: u8, id: Option<&str>, classes: &[&str]) -> String {
        let class_attr = if classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", classes.join(" "))
        };

        let Some(id) = id else {
            return format!("<h{level}{class_attr}>");
        };

        format!(
            "<h{level} id=\"{id}\"{class_attr}><a class=\"docs-heading-anchor\" href=\"#{id}\">{ANCHOR_ICON}<span class=\"sr-only\">Link to this heading</span></a>"
        )
    }

    fn render_end(&self, level: u8) -> String {
        format!("</h{level}>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_an_anchor_for_headings_with_ids() {
        let heading = DocsHeading;
        let output = heading.render_start(2, Some("some-heading"), &[]);

        assert!(output.starts_with(
            "<h2 id=\"some-heading\"><a class=\"docs-heading-anchor\" href=\"#some-heading\">"
        ));
        assert!(output.contains(
            "<span class=\"docs-heading-symbol\" aria-hidden=\"true\">#</span>"
        ));
        assert!(output.contains("<span class=\"sr-only\">Link to this heading</span>"));
        assert_eq!(heading.render_end(2), "</h2>");
    }

    #[test]
    fn renders_plain_headings_without_ids() {
        assert_eq!(DocsHeading.render_start(3, None, &[]), "<h3>");
    }

    #[test]
    fn renders_heading_classes() {
        let output =
            DocsHeading.render_start(2, Some("some-heading"), &["class-one", "class-two"]);

        assert!(output.starts_with(
            "<h2 id=\"some-heading\" class=\"class-one class-two\"><a class=\"docs-heading-anchor\""
        ));
    }
}
