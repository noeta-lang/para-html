//! The `para/html` package's **tier-body formatters** for `noeta fmt` — HTML reflow plus CSS (via
//! [`malva`](https://docs.rs/malva)), shipped as a native, dev-only extension.
//!
//! This is the extension-driven tier-body-formatting story: `@html` is a *program* tier (declared in
//! the in-language liveview surface, with a reactive Noeta handler), and this crate is a
//! formatter-only pair of [`Extension`]s that register native re-indenters for the `"html"` and
//! `"css"` **languages**. Any tier — program or native — that declares `text: "html"` gets it; `fmt`
//! resolves by language and delegates. Core stays HTML-ignorant, the `@html` handler stays idiomatic
//! Noeta, and the formatter lives where the language knowledge does: here, in the package's extension.
//!
//! The formatter is a *pure foreign reflow*: `fmt` hands it the body's HTML with each `${…}` hole
//! collapsed to a single NUL (`\0`) placeholder plus the `indent` to lay the top level at, and takes
//! back reflowed HTML with the NULs in the same order — `fmt` substitutes the (inline-formatted)
//! holes and re-applies tier-body escaping. So this file never sees Noeta syntax; it only
//! pretty-prints HTML (and delegates `<style>` bodies to the CSS formatter).
//!
//! ## Dev-only, trust-free, feature-gated
//!
//! The whole formatter path (the HTML reflow *and* malva) sits behind the `fmt` cargo feature. A
//! **default build ships two EMPTY extensions** — no malva, `body_formatters()` returns `&[]` — so a
//! consumer that pulls in `para/html` links no formatter and authorizes no native trust. The dev
//! toolchain builds this crate with `--features fmt` to reflow HTML/CSS under `noeta fmt`; a prod
//! build never does.

use noeta_ext_abi::registry::{BodyFormatter, ExtModule, Extension};

/// The formatter-only HTML extension. It contributes no modules or types — its whole purpose is to
/// register the `"html"` body formatter so `@html` (and any `text: "html"` tier) reflows under
/// `noeta fmt`. Its own namespace root is `"html"`, distinct from `std`.
#[derive(Debug)]
pub struct HtmlExtension;

/// A process-static handle the composed dev toolchain assembles into its `noeta fmt` registry.
pub static HTML_EXTENSION: HtmlExtension = HtmlExtension;

/// The formatter-only CSS extension (namespace root `"css"`, distinct from `std`), backed by malva.
#[derive(Debug)]
pub struct CssExtension;

/// A process-static handle the composed dev toolchain assembles into its `noeta fmt` registry.
pub static CSS_EXTENSION: CssExtension = CssExtension;

/// The package's extension slice, aggregated by the native entry crate under the fixed
/// `NOETA_EXTENSIONS` convention and installed into the dev toolchain's `noeta fmt` registry.
pub static NOETA_EXTENSIONS: &[&(dyn Extension + Sync)] = &[&HTML_EXTENSION, &CSS_EXTENSION];

#[cfg(feature = "fmt")]
const HTML_FORMATTERS: &[BodyFormatter] = &[("html", html_reindent)];

#[cfg(feature = "fmt")]
const CSS_FORMATTERS: &[BodyFormatter] = &[("css", css_format)];

impl Extension for HtmlExtension {
    fn name(&self) -> &'static str {
        "html"
    }
    fn modules(&self) -> &'static [ExtModule] {
        &[]
    }
    fn body_formatters(&self) -> &'static [BodyFormatter] {
        // Default build = empty extension: the formatter path (and malva) is gated on `fmt`.
        #[cfg(feature = "fmt")]
        {
            HTML_FORMATTERS
        }
        #[cfg(not(feature = "fmt"))]
        {
            &[]
        }
    }
}

impl Extension for CssExtension {
    fn name(&self) -> &'static str {
        "css"
    }
    fn modules(&self) -> &'static [ExtModule] {
        &[]
    }
    fn body_formatters(&self) -> &'static [BodyFormatter] {
        // Default build = empty extension: the malva-backed formatter is gated on `fmt`.
        #[cfg(feature = "fmt")]
        {
            CSS_FORMATTERS
        }
        #[cfg(not(feature = "fmt"))]
        {
            &[]
        }
    }
}

// ============================================================================================
// HTML reflow (feature `fmt`).
// ============================================================================================

/// HTML elements whose content is laid out as **block** structure — each such open/close tag gets its
/// own line, and its children are indented. Everything else (`span`, `b`, `a`, …) is treated as
/// **inline** and flows on the current line, so `<b>${x}</b>` and `[x] ${title}` stay together. (A
/// pragmatic, not exhaustive, list — the common structural elements. `button` is included: templates
/// use it as a standalone control.)
#[cfg(feature = "fmt")]
const BLOCK: &[&str] = &[
    "html",
    "head",
    "body",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "main",
    "aside",
    "p",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "form",
    "fieldset",
    "figure",
    "blockquote",
    "hr",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "button",
];

/// Void elements — no closing tag, no children — emitted inline as atoms.
#[cfg(feature = "fmt")]
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// **Raw-text** / whitespace-significant elements: their content is preserved **byte-for-byte** —
/// no HTML tokenizing, no whitespace collapse, no re-indentation. `pre`/`textarea` render whitespace
/// literally; `script`/`style` carry foreign code HTML must not touch. The open tag is laid out like
/// a block, but everything up to and including the matching close tag is emitted verbatim.
#[cfg(feature = "fmt")]
const RAW: &[&str] = &["pre", "textarea", "script", "style"];

/// The one-line width above which a block whose children are all **inline** reflows those children
/// onto their own lines (at existing structural whitespace only). Matches `FmtConfig`'s default line
/// width; a deliberate heuristic — holes are 1-char `\0` during reindent, so exact width is
/// impossible, and threading the real config would be an ABI change.
#[cfg(feature = "fmt")]
const HTML_LINE_WIDTH: usize = 100;

#[cfg(feature = "fmt")]
fn is_block(name: &str) -> bool {
    BLOCK.contains(&name)
}
#[cfg(feature = "fmt")]
fn is_void(name: &str) -> bool {
    VOID.contains(&name)
}
#[cfg(feature = "fmt")]
fn is_raw(name: &str) -> bool {
    RAW.contains(&name)
}

/// The NUL-free control markers that bracket a **raw region** inside the intermediate (pre-indent)
/// layout. They never reach `fmt`: the final indentation pass emits the region verbatim and strips
/// them. (`\0` is reserved for holes.)
#[cfg(feature = "fmt")]
const RAW_OPEN: char = '\u{11}';
#[cfg(feature = "fmt")]
const RAW_CLOSE: char = '\u{12}';

#[cfg(feature = "fmt")]
enum Tok {
    /// An opening (or self-closing) tag: its verbatim `<…>` text, lowercased name, and whether it is
    /// self-closing or a void element.
    Open {
        name: String,
        raw: String,
        self_closing: bool,
        void: bool,
    },
    /// A closing `</…>` tag.
    Close { name: String, raw: String },
    /// A raw-text element captured whole: its lowercased name, open tag, verbatim inner content
    /// (holes still `\0`), and close tag — none of which is reflowed by the HTML pass (though a
    /// `<style>`/`<script>` body may be delegated to a CSS/JS formatter).
    Raw {
        name: String,
        open: String,
        content: String,
        close: String,
    },
    /// A run of text between tags/holes.
    Text(String),
    /// A `${…}` hole, carried as a single NUL by `fmt`.
    Hole,
}

/// The end index of a `<…>` tag starting at `open` (the position of `<`), skipping any `>` inside a
/// quoted attribute value. Returns the index of the closing `>`, or `None` if unterminated.
#[cfg(feature = "fmt")]
fn tag_end(chars: &[char], open: usize) -> Option<usize> {
    let mut i = open + 1;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(i),
            None => {}
        }
        i += 1;
    }
    None
}

/// The start index of the matching `</name …>` close tag at or after `from` (case-insensitive tag
/// name), for capturing a raw element's content. `None` if the element is never closed.
#[cfg(feature = "fmt")]
fn find_close(chars: &[char], from: usize, name: &str) -> Option<usize> {
    let needle: Vec<char> = format!("</{name}").chars().collect();
    let mut i = from;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()]
            .iter()
            .zip(&needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            // The name must end here (next char is `>`, whitespace, or `/`), not be a longer name.
            if matches!(
                chars.get(i + needle.len()),
                Some('>' | ' ' | '\t' | '\n' | '\r' | '/')
            ) {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(feature = "fmt")]
fn tag_name(inner: &str) -> String {
    inner
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Tokenize HTML (with `\0` holes) into tags / text / holes / raw elements. `None` on an unterminated
/// tag or an unclosed raw element — the signal for the formatter to decline and leave the body
/// verbatim rather than emit broken markup.
#[cfg(feature = "fmt")]
fn tokenize(body: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = body.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    let mut text = String::new();
    let flush = |text: &mut String, toks: &mut Vec<Tok>| {
        if !text.is_empty() {
            toks.push(Tok::Text(std::mem::take(text)));
        }
    };
    while i < chars.len() {
        match chars[i] {
            '\u{0}' => {
                flush(&mut text, &mut toks);
                toks.push(Tok::Hole);
                i += 1;
            }
            '<' => {
                flush(&mut text, &mut toks);
                let end = tag_end(&chars, i)?;
                let raw: String = chars[i..=end].iter().collect(); // `<` … `>`
                let inner = &raw[1..raw.len() - 1];
                let is_close = inner.starts_with('/');
                let self_closing = inner.ends_with('/');
                let name = tag_name(inner);
                i = end + 1;
                if is_close {
                    toks.push(Tok::Close { name, raw });
                } else if !self_closing && is_raw(&name) {
                    // Capture the element whole — content byte-for-byte to the matching close tag.
                    let close_start = find_close(&chars, i, &name)?;
                    let content: String = chars[i..close_start].iter().collect();
                    let close_end = tag_end(&chars, close_start)?;
                    let close: String = chars[close_start..=close_end].iter().collect();
                    i = close_end + 1;
                    toks.push(Tok::Raw {
                        name,
                        open: raw,
                        content,
                        close,
                    });
                } else {
                    let void = is_void(&name);
                    toks.push(Tok::Open {
                        name,
                        raw,
                        self_closing,
                        void,
                    });
                }
            }
            _ => {
                text.push(chars[i]);
                i += 1;
            }
        }
    }
    flush(&mut text, &mut toks);
    Some(toks)
}

/// Collapse every run of ASCII whitespace to a single space (HTML's own whitespace model). Leading
/// and trailing spaces are kept — they carry inline spacing like `${box} ${title}` — but a purely
/// structural gap collapses to a lone space, trimmed off at the next break.
#[cfg(feature = "fmt")]
fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}

/// Re-indent HTML by block-element nesting, laying the top level at `base`. A block element opens
/// structure (children indent); an element with only inline content stays on one line
/// (`<li class="x">[x] \0</li>`), one with block children breaks its close tag onto its own line. A
/// block whose children are all **inline** but whose one-line form would exceed [`HTML_LINE_WIDTH`]
/// reflows those children onto their own lines — but only at *existing* structural whitespace, never
/// where there is none (so `</span><span>` never gains a break). Inline elements, text, and `\0`
/// holes flow inline. Raw-text elements (`<pre>`, `<textarea>`, `<script>`, `<style>`) keep their
/// content byte-for-byte, unindented and uncollapsed. Idempotent; declines (`None` → verbatim) on
/// unterminated or unclosed markup.
#[cfg(feature = "fmt")]
pub fn html_reindent(
    body: &str,
    base: &str,
    sub: &dyn Fn(&str, &str, &str) -> Option<String>,
) -> Option<String> {
    let toks = tokenize(body)?;
    // Pass 1: a relative layout (2-space nesting, column 0), with raw regions bracketed by control
    // markers. Trailing spaces are trimmed at each break, so only raw content can hold them.
    let mut buf = String::new();
    let mut depth = 0usize;
    // Per-open-block bookkeeping (parallel stacks, pushed at each `Tok::Open` of a block):
    //  - `had_block_child`: did this block contain a block child / raw element? (→ break its close).
    //  - `break_points`: buf offsets of STRUCTURAL single-space gaps between this block's inline
    //    children — the ONLY places a width-driven reflow may break.
    //  - `content_start`: the buf offset where this block's inner content begins (after its open tag).
    let mut had_block_child: Vec<bool> = Vec::new();
    let mut break_points: Vec<Vec<usize>> = Vec::new();
    let mut content_start: Vec<usize> = Vec::new();
    let br = |buf: &mut String, depth: usize| {
        while buf.ends_with(' ') {
            buf.pop();
        }
        buf.push('\n');
        for _ in 0..depth {
            buf.push_str("  ");
        }
    };
    for tok in toks {
        match tok {
            Tok::Open {
                name,
                raw,
                self_closing,
                void,
            } => {
                if void || self_closing || !is_block(&name) {
                    buf.push_str(&raw);
                } else {
                    if let Some(top) = had_block_child.last_mut() {
                        *top = true;
                    }
                    if !buf.is_empty() {
                        br(&mut buf, depth);
                    }
                    buf.push_str(&raw);
                    had_block_child.push(false);
                    break_points.push(Vec::new());
                    content_start.push(buf.len());
                    depth += 1;
                }
            }
            Tok::Close { name, raw } => {
                if is_block(&name) {
                    depth = depth.saturating_sub(1);
                    let had_block = had_block_child.pop().unwrap_or(false);
                    let bps = break_points.pop().unwrap_or_default();
                    let start = content_start.pop().unwrap_or(buf.len());
                    if had_block {
                        // Block children already broke; put the close tag on its own line.
                        br(&mut buf, depth);
                        buf.push_str(&raw);
                    } else {
                        // Inline-only block: measure the prospective ONE-LINE width (the collapsed
                        // form currently in `buf`, invariant under reformatting) and reflow only if it
                        // overflows AND there is existing structural whitespace to break at.
                        let line_start = buf.rfind('\n').map(|i| i + 1).unwrap_or(0);
                        let line_len = buf[line_start..].chars().count();
                        let width = base.chars().count() + line_len + raw.chars().count();
                        if !bps.is_empty() && width > HTML_LINE_WIDTH {
                            // Reflow: replace each recorded break-point space with a newline + child
                            // indent, then the close tag on its own line at the parent depth — exactly
                            // the layout the `had_block == true` path produces.
                            let child_indent = "  ".repeat(depth + 1);
                            let mut broken = String::new();
                            let mut prev = start;
                            for bp in &bps {
                                broken.push_str(&buf[prev..*bp]);
                                broken.push('\n');
                                broken.push_str(&child_indent);
                                prev = *bp + 1; // skip the single-space gap
                            }
                            broken.push_str(&buf[prev..]);
                            // Trim any trailing break (a gap right before the close tag) so a second
                            // run — which re-tokenizes that newline to a pure-whitespace gap — lands
                            // on the identical layout. This is what makes the reflow idempotent.
                            let broken = broken.trim_end();
                            buf.truncate(start);
                            buf.push_str(broken);
                            br(&mut buf, depth);
                            buf.push_str(&raw);
                        } else {
                            buf.push_str(&raw);
                        }
                    }
                } else {
                    buf.push_str(&raw);
                }
            }
            Tok::Raw {
                name,
                open,
                content,
                close,
            } => {
                if let Some(top) = had_block_child.last_mut() {
                    *top = true;
                }
                if !buf.is_empty() {
                    br(&mut buf, depth);
                }
                buf.push_str(&open);
                // A `<style>`/`<script>` whose content has no `${…}` hole is delegated to the
                // registered `"css"`/`"javascript"` formatter — the content laid out at one level
                // under the tag. If none is registered (or it declines, or a hole is present), the
                // content stays byte-for-byte verbatim.
                let language = match name.as_str() {
                    "style" => Some("css"),
                    "script" => Some("javascript"),
                    _ => None,
                };
                let delegated = language
                    .filter(|_| !content.contains('\u{0}') && !content.trim().is_empty())
                    .and_then(|lang| {
                        // One level deeper than the tag, which sits at `base` + its nesting depth.
                        let inner = format!("{base}{}", "  ".repeat(depth + 1));
                        sub(lang, content.trim(), &inner)
                    });
                match delegated {
                    Some(formatted) => {
                        // The formatted sub-language (already indented at `base + one level`) sits on
                        // its own lines in a raw region; the close tag returns to the tag's own depth.
                        buf.push(RAW_OPEN);
                        buf.push('\n');
                        buf.push_str(&formatted);
                        buf.push(RAW_CLOSE);
                        br(&mut buf, depth);
                        buf.push_str(&close);
                    }
                    None => {
                        buf.push(RAW_OPEN);
                        buf.push_str(&content); // byte-for-byte
                        buf.push_str(&close);
                        buf.push(RAW_CLOSE);
                    }
                }
            }
            Tok::Text(t) => {
                let collapsed = collapse_ws(&t);
                // A break point is recorded ONLY when the whole text token is pure ASCII whitespace
                // that collapses to a single structural space between (inline) items — never inside a
                // meaningful run like `"[x] \0"`, and never inside a tag's raw/attributes (those are
                // Open/Close tokens, not Text). This guarantees a reflow only ever breaks where a gap
                // already exists.
                if collapsed == " " && !t.is_empty() && t.chars().all(|c| c.is_ascii_whitespace()) {
                    buf.push(' ');
                    if let Some(bps) = break_points.last_mut() {
                        bps.push(buf.len() - 1);
                    }
                } else {
                    buf.push_str(&collapsed);
                }
            }
            Tok::Hole => buf.push('\u{0}'),
        }
    }
    // Pass 2: prepend `base` to each line — except lines inside a raw region, which are emitted
    // verbatim — and strip the raw markers. Leading whitespace (the body's own indentation before
    // the first element) is dropped so the body has no blank first line under `@<tier> {`.
    let mut out = String::new();
    let mut in_raw = false;
    let mut at_line_start = true;
    for c in buf.trim_start().chars() {
        match c {
            RAW_OPEN => in_raw = true,
            RAW_CLOSE => in_raw = false,
            '\n' => {
                out.push('\n');
                at_line_start = true;
            }
            _ => {
                if at_line_start {
                    if !in_raw {
                        out.push_str(base);
                    }
                    at_line_start = false;
                }
                out.push(c);
            }
        }
    }
    Some(out.trim_end().to_string())
}

// ============================================================================================
// CSS reflow via malva (feature `fmt`).
// ============================================================================================

/// Format a `<style>` body — plain CSS (the HTML formatter only delegates hole-free content, so there
/// are no `${…}` holes to preserve) — with malva, then indent every line at `indent` so it nests
/// under the tag. Declines (`None` → the HTML formatter leaves the body verbatim) on a parse error.
#[cfg(feature = "fmt")]
fn css_format(
    body: &str,
    indent: &str,
    _sub: &dyn Fn(&str, &str, &str) -> Option<String>,
) -> Option<String> {
    let options = malva::config::FormatOptions::default();
    let formatted = malva::format_text(body, malva::Syntax::Css, &options).ok()?;
    // malva lays CSS out from column 0; place it under the tag by prefixing `indent` to each
    // non-empty line (blank lines stay empty so they carry no trailing indentation).
    let indented = formatted
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(indented.trim_end().to_string())
}

#[cfg(all(test, feature = "fmt"))]
mod tests {
    use super::*;

    /// Reindent with no sub-formatters registered (so `<style>`/`<script>` stay verbatim).
    fn fmt(html: &str) -> String {
        html_reindent(html, "", &|_, _, _| None).expect("well-formed")
    }

    /// A sub-formatter stub that "formats" CSS by uppercasing it (indented at `indent`), to prove
    /// delegation without depending on a real CSS formatter.
    fn fmt_with_css(html: &str) -> String {
        let sub = |lang: &str, body: &str, indent: &str| {
            (lang == "css").then(|| {
                body.lines()
                    .map(|l| format!("{indent}{}", l.trim().to_uppercase()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        };
        html_reindent(html, "", &sub).expect("well-formed")
    }

    #[test]
    fn block_children_indent_inline_content_stays() {
        let out = fmt("<ul><li class=\"todo\">[x] \u{0}</li><li>\u{0}</li></ul>");
        assert_eq!(
            out,
            "<ul>\n  <li class=\"todo\">[x] \u{0}</li>\n  <li>\u{0}</li>\n</ul>"
        );
    }

    #[test]
    fn base_indent_is_applied_to_every_structural_line() {
        let out = html_reindent("<ul><li>a</li></ul>", "    ", &|_, _, _| None).expect("ok");
        assert_eq!(out, "    <ul>\n      <li>a</li>\n    </ul>");
    }

    #[test]
    fn style_body_is_delegated_to_the_css_formatter() {
        // With a "css" sub-formatter, a `<style>` body is reflowed by it (here: uppercased, indented
        // one level under `<style>`); the `<style>`/`</style>` tags keep HTML block layout.
        let out = fmt_with_css("<div><style>a{color:red}</style></div>");
        assert_eq!(
            out,
            "<div>\n  <style>\n    A{COLOR:RED}\n  </style>\n</div>"
        );
    }

    #[test]
    fn style_with_a_hole_stays_verbatim() {
        // A `${…}` hole (NUL) in the body means it can't be handed to a plain CSS formatter, so it is
        // left byte-for-byte even when a "css" formatter is registered.
        let out = fmt_with_css("<style>a{color:\u{0}}</style>");
        assert_eq!(out, "<style>a{color:\u{0}}</style>");
    }

    #[test]
    fn pre_content_is_verbatim_uncollapsed_and_unindented() {
        // The whitespace inside <pre> is significant: it survives byte-for-byte, gets no base indent,
        // and is not collapsed — even though the <pre> tag itself is indented as a block.
        let out = html_reindent(
            "<div><pre>  keep\n    these   spaces\n</pre></div>",
            "",
            &|_, _, _| None,
        )
        .expect("ok");
        assert_eq!(
            out,
            "<div>\n  <pre>  keep\n    these   spaces\n</pre>\n</div>"
        );
    }

    #[test]
    fn holes_inside_pre_are_preserved() {
        let out = fmt("<pre>x = \u{0}\n</pre>");
        assert_eq!(out, "<pre>x = \u{0}\n</pre>");
    }

    #[test]
    fn is_idempotent() {
        let once =
            fmt("<div><p>hi <b>\u{0}</b></p><ul><li>a</li></ul><pre>  raw\n  text\n</pre></div>");
        assert_eq!(fmt(&once), once, "html reindent is not idempotent");
    }

    #[test]
    fn holes_in_attributes_are_preserved_in_order() {
        let out = fmt("<a href=\"\u{0}\">click \u{0}</a>");
        assert_eq!(out.matches('\u{0}').count(), 2);
        assert!(out.starts_with("<a href=\"\u{0}\">"));
    }

    #[test]
    fn unterminated_tag_declines() {
        assert!(html_reindent("<div class=\"x", "", &|_, _, _| None).is_none());
        assert!(html_reindent("<pre>never closed", "", &|_, _, _| None).is_none());
        assert!(html_reindent("<div>oops", "", &|_, _, _| None).is_some());
    }

    // ---- CSS formatter (malva) ----

    #[test]
    fn reindents_css_under_the_given_base() {
        let out = css_format("a{color:red;background:blue}", "  ", &|_, _, _| None).unwrap();
        // malva normalizes the rule; every non-blank line is indented at least two spaces.
        assert!(
            out.lines().all(|l| l.is_empty() || l.starts_with("  ")),
            "got:\n{out}"
        );
        assert!(out.contains("color: red"), "got:\n{out}");
    }

    #[test]
    fn declines_on_a_parse_error() {
        assert!(css_format("a { color: ", "", &|_, _, _| None).is_none());
    }

    // ---- Width-aware reflow of an inline-only block (the width-collapse bug fix) ----

    /// A long `<li>` whose children are all inline `<span>`s (its one-line form exceeds
    /// `HTML_LINE_WIDTH`) breaks each span onto its own indented line, with `</li>` on its own line —
    /// the collapse bug: before the fix this stayed on a single ~150-char line.
    #[test]
    fn long_inline_only_block_reflows_at_whitespace() {
        let input = "<li><span>alpha item one</span> <span>bravo item two</span> \
                     <span>charlie item three</span> <span>delta item four</span></li>";
        assert!(
            input.chars().count() > HTML_LINE_WIDTH,
            "fixture must exceed the width to trigger the branch"
        );
        let out = fmt(input);
        assert_eq!(
            out,
            "<li><span>alpha item one</span>\n  \
             <span>bravo item two</span>\n  \
             <span>charlie item three</span>\n  \
             <span>delta item four</span>\n</li>"
        );
    }

    /// The SAME content with NO whitespace between the spans (`</span><span>`) stays on one line even
    /// past the width: there is no existing gap to break at, so the formatter never invents one.
    #[test]
    fn long_inline_only_block_without_gaps_stays_one_line() {
        let input = "<li><span>alpha item one</span><span>bravo item two</span>\
                     <span>charlie item three</span><span>delta item four</span></li>";
        assert!(input.chars().count() > HTML_LINE_WIDTH);
        let out = fmt(input);
        // No Text token exists between adjacent tags, so no break point → unchanged single line.
        assert_eq!(out, input);
    }

    /// The reflowed long-`<li>` is idempotent: formatting the broken output again re-tokenizes the
    /// inter-span newlines to pure-whitespace gaps that collapse back to single spaces at the same
    /// offsets → same width → same decision → stable.
    #[test]
    fn long_inline_only_block_is_idempotent() {
        let input = "<li><span>alpha item one</span> <span>bravo item two</span> \
                     <span>charlie item three</span> <span>delta item four</span></li>";
        let once = fmt(input);
        assert_eq!(fmt(&once), once, "width-reflow is not idempotent");
    }
}
