use scraper::ElementRef;
use scraper::node::Node;

pub fn html_to_mrkdwn(elem: ElementRef<'_>) -> String {
    let mut out = String::new();
    render_children(elem, &mut out, Ctx::default());
    normalize(out)
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    in_pre: bool,
}

fn render_children(elem: ElementRef<'_>, out: &mut String, ctx: Ctx) {
    for child in elem.children() {
        match child.value() {
            Node::Text(t) => {
                if ctx.in_pre {
                    out.push_str(t);
                } else {
                    out.push_str(&escape_text(t));
                }
            }
            Node::Element(_) => {
                if let Some(child_elem) = ElementRef::wrap(child) {
                    render_element(child_elem, out, ctx);
                }
            }
            _ => {}
        }
    }
}

fn render_element(elem: ElementRef<'_>, out: &mut String, ctx: Ctx) {
    match elem.value().name() {
        "p" => {
            render_children(elem, out, ctx);
            out.push_str("\n\n");
        }
        "strong" | "b" => {
            out.push('*');
            render_children(elem, out, ctx);
            out.push('*');
        }
        "em" | "i" => {
            out.push('_');
            render_children(elem, out, ctx);
            out.push('_');
        }
        "s" | "strike" | "del" => {
            out.push('~');
            render_children(elem, out, ctx);
            out.push('~');
        }
        "br" => out.push('\n'),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            out.push('*');
            render_children(elem, out, ctx);
            out.push_str("*\n");
        }
        "a" => {
            let text = plain_text(elem);
            let text = text.trim();
            if let Some(href) = elem.value().attr("href") {
                if text.is_empty() || text == href {
                    out.push_str(&format!("<{href}>"));
                } else {
                    out.push_str(&format!("<{href}|{text}>"));
                }
            } else {
                out.push_str(text);
            }
        }
        "ul" => {
            for child in elem.children().filter_map(ElementRef::wrap) {
                if child.value().name() == "li" {
                    out.push_str("• ");
                    render_children(child, out, ctx);
                    out.push('\n');
                }
            }
            out.push('\n');
        }
        "ol" => {
            for (i, child) in elem
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|e| e.value().name() == "li")
                .enumerate()
            {
                out.push_str(&format!("{}. ", i + 1));
                render_children(child, out, ctx);
                out.push('\n');
            }
            out.push('\n');
        }
        "pre" => {
            out.push_str("```\n");
            render_children(elem, out, Ctx { in_pre: true });
            out.push_str("\n```\n\n");
        }
        "code" if !ctx.in_pre => {
            let code = plain_text(elem);
            out.push('`');
            out.push_str(&code);
            out.push('`');
        }
        _ => render_children(elem, out, ctx),
    }
}

fn plain_text(elem: ElementRef<'_>) -> String {
    elem.text().collect::<Vec<_>>().join("")
}

fn escape_text(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '&' => vec!['&', 'a', 'm', 'p', ';'],
            '<' => vec!['&', 'l', 't', ';'],
            '>' => vec!['&', 'g', 't', ';'],
            _ => vec![c],
        })
        .collect()
}

fn normalize(mut s: String) -> String {
    // Collapse 3+ newlines into 2
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim().to_string()
}
