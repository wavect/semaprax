//! Bounded YAML subset for Agent Skills front matter.
//!
//! Built on the maintained `yaml-rust2` event parser (no in-house YAML). The
//! parser is pulled event by event so every hostile construct is refused
//! before anything is materialised: aliases, anchors, tags, multiple
//! documents, non-scalar keys, duplicate keys, and over-deep or over-large
//! documents. Scalars stay strings (`1.0` is `"1.0"`, `true` is `"true"`).

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::TScalarStyle;

/// Hard bounds applied to one front-matter document.
#[derive(Clone, Copy, Debug)]
pub struct YamlLimits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl YamlLimits {
    pub const FRONT_MATTER: YamlLimits = YamlLimits {
        max_bytes: 16 * 1024,
        max_depth: 6,
        max_nodes: 512,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Yaml {
    Str(String),
    Seq(Vec<Yaml>),
    /// Insertion-ordered; keys are unique.
    Map(Vec<(String, Yaml)>),
}

/// Why a document was refused; the caller maps the kind to a stable code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YamlError {
    /// Syntax error.
    Syntax(String),
    /// Alias, anchor, tag, multi-document or non-scalar key.
    Unsafe(String),
    /// A bound in [`YamlLimits`] was exceeded.
    Bound(String),
    /// Duplicate mapping key.
    Duplicate(String),
}

enum Frame {
    Seq(Vec<Yaml>),
    Map {
        entries: Vec<(String, Yaml)>,
        key: Option<String>,
    },
}

/// Parse one YAML document into the bounded tree. An empty document is an
/// empty map.
pub fn parse(text: &str, lim: &YamlLimits) -> Result<Yaml, YamlError> {
    if text.len() > lim.max_bytes {
        return Err(YamlError::Bound(format!(
            "front matter exceeds {} bytes",
            lim.max_bytes
        )));
    }
    let mut parser = Parser::new_from_str(text);
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<Yaml> = None;
    let mut docs = 0usize;
    let mut nodes = 0usize;
    loop {
        let (ev, _) = parser
            .next_token()
            .map_err(|e| YamlError::Syntax(e.to_string()))?;
        match ev {
            Event::Nothing | Event::StreamStart => {}
            Event::DocumentStart => {
                docs += 1;
                if docs > 1 {
                    return Err(YamlError::Unsafe("multiple YAML documents".into()));
                }
            }
            Event::DocumentEnd => {}
            Event::StreamEnd => break,
            Event::Alias(_) => return Err(YamlError::Unsafe("YAML aliases are refused".into())),
            Event::Scalar(v, style, anchor, tag) => {
                refuse_decorations(anchor, tag.is_some())?;
                nodes += 1;
                bound_nodes(nodes, lim)?;
                let v = if style == TScalarStyle::Plain && v == "~" {
                    String::new()
                } else {
                    v
                };
                place(&mut stack, &mut root, Yaml::Str(v), true)?;
            }
            Event::SequenceStart(anchor, tag) => {
                refuse_decorations(anchor, tag.is_some())?;
                nodes += 1;
                bound_nodes(nodes, lim)?;
                refuse_key_position(&stack)?;
                stack.push(Frame::Seq(Vec::new()));
                bound_depth(&stack, lim)?;
            }
            Event::MappingStart(anchor, tag) => {
                refuse_decorations(anchor, tag.is_some())?;
                nodes += 1;
                bound_nodes(nodes, lim)?;
                refuse_key_position(&stack)?;
                stack.push(Frame::Map {
                    entries: Vec::new(),
                    key: None,
                });
                bound_depth(&stack, lim)?;
            }
            Event::SequenceEnd => {
                let Some(Frame::Seq(items)) = stack.pop() else {
                    return Err(YamlError::Syntax("unbalanced sequence".into()));
                };
                place(&mut stack, &mut root, Yaml::Seq(items), false)?;
            }
            Event::MappingEnd => {
                let Some(Frame::Map { entries, key: None }) = stack.pop() else {
                    return Err(YamlError::Syntax("mapping key without a value".into()));
                };
                place(&mut stack, &mut root, Yaml::Map(entries), false)?;
            }
        }
    }
    Ok(root.unwrap_or(Yaml::Map(Vec::new())))
}

fn refuse_decorations(anchor: usize, tag: bool) -> Result<(), YamlError> {
    if anchor != 0 {
        return Err(YamlError::Unsafe("YAML anchors are refused".into()));
    }
    if tag {
        return Err(YamlError::Unsafe("YAML tags are refused".into()));
    }
    Ok(())
}

fn bound_nodes(n: usize, lim: &YamlLimits) -> Result<(), YamlError> {
    if n > lim.max_nodes {
        return Err(YamlError::Bound(format!(
            "front matter has more than {} nodes",
            lim.max_nodes
        )));
    }
    Ok(())
}

fn bound_depth(stack: &[Frame], lim: &YamlLimits) -> Result<(), YamlError> {
    if stack.len() > lim.max_depth {
        return Err(YamlError::Bound(format!(
            "front matter nests deeper than {}",
            lim.max_depth
        )));
    }
    Ok(())
}

/// A collection may not start where a mapping key is expected.
fn refuse_key_position(stack: &[Frame]) -> Result<(), YamlError> {
    if matches!(stack.last(), Some(Frame::Map { key: None, .. })) {
        return Err(YamlError::Unsafe(
            "non-scalar mapping keys are refused".into(),
        ));
    }
    Ok(())
}

fn place(
    stack: &mut [Frame],
    root: &mut Option<Yaml>,
    v: Yaml,
    scalar: bool,
) -> Result<(), YamlError> {
    match stack.last_mut() {
        None => {
            if root.is_some() {
                return Err(YamlError::Syntax("more than one root node".into()));
            }
            *root = Some(v);
        }
        Some(Frame::Seq(items)) => items.push(v),
        Some(Frame::Map { entries, key }) => match key.take() {
            None => {
                let Yaml::Str(k) = v else {
                    return Err(YamlError::Unsafe(
                        "non-scalar mapping keys are refused".into(),
                    ));
                };
                debug_assert!(scalar);
                if entries.iter().any(|(e, _)| *e == k) {
                    return Err(YamlError::Duplicate(k));
                }
                *key = Some(k);
            }
            Some(k) => entries.push((k, v)),
        },
    }
    Ok(())
}

impl Yaml {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Yaml::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Canonical JSON rendering, used to keep inert extension data.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Yaml::Str(s) => serde_json::Value::String(s.clone()),
            Yaml::Seq(v) => serde_json::Value::Array(v.iter().map(Yaml::to_json).collect()),
            Yaml::Map(m) => {
                serde_json::Value::Object(m.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
        }
    }
}
