//! 把 `Accessibility.getFullAXTree` 的结果格式化成带 `ref=eN` 的缩进文本，
//! 同时产出 ref → backendDOMNodeId 映射，供 click / type 等按 ref 定位元素。

use std::collections::HashMap;

use serde_json::Value;

use super::cdp::SnapshotRefs;

const MAX_LINES: usize = 2000;

struct AxNode {
    role: String,
    name: String,
    backend_id: Option<i64>,
    ignored: bool,
    child_ids: Vec<String>,
    parent_id: Option<String>,
}

fn field_value(node: &Value, key: &str) -> String {
    node.get(key)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// 返回 (人类可读文本, ref 映射)。
pub fn format_ax_tree(result: &Value) -> (String, SnapshotRefs) {
    let empty = Vec::new();
    let raw_nodes = result.get("nodes").and_then(|n| n.as_array()).unwrap_or(&empty);

    let mut nodes: HashMap<String, AxNode> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for n in raw_nodes {
        let Some(id) = n.get("nodeId").and_then(|v| v.as_str()) else {
            continue;
        };
        let role = field_value(n, "role");
        let name = field_value(n, "name");
        let backend_id = n.get("backendDOMNodeId").and_then(|v| v.as_i64());
        let ignored = n.get("ignored").and_then(|v| v.as_bool()).unwrap_or(false);
        let child_ids = n
            .get("childIds")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let parent_id = n
            .get("parentId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        order.push(id.to_string());
        nodes.insert(
            id.to_string(),
            AxNode {
                role,
                name,
                backend_id,
                ignored,
                child_ids,
                parent_id,
            },
        );
    }

    // 根：没有 parentId 的节点（通常是 RootWebArea）。
    let roots: Vec<String> = order
        .iter()
        .filter(|id| nodes.get(*id).and_then(|n| n.parent_id.as_ref()).is_none())
        .cloned()
        .collect();

    let mut out = String::new();
    let mut refs = SnapshotRefs::default();
    let mut counter = 0usize;
    let mut lines = 0usize;

    let mut stack: Vec<(String, usize)> = Vec::new();
    // 逆序压栈以保持子节点自然顺序
    for r in roots.iter().rev() {
        stack.push((r.clone(), 0));
    }

    while let Some((id, depth)) = stack.pop() {
        if lines >= MAX_LINES {
            out.push_str("... (truncated)\n");
            break;
        }
        let Some(node) = nodes.get(&id) else { continue };

        // 可见（非 ignored 且有 role）的节点才输出；ignored 容器仍下探其子。
        let mut next_depth = depth;
        if !node.ignored && !node.role.is_empty() {
            let mut line = "  ".repeat(depth);
            line.push_str("- ");
            line.push_str(&node.role);
            if !node.name.is_empty() {
                let name = node.name.replace('\n', " ");
                let name: String = name.chars().take(120).collect();
                line.push_str(&format!(" \"{name}\""));
            }
            if let Some(bid) = node.backend_id {
                counter += 1;
                let r = format!("e{counter}");
                refs.map.insert(r.clone(), bid);
                line.push_str(&format!(" [ref={r}]"));
            }
            out.push_str(&line);
            out.push('\n');
            lines += 1;
            next_depth = depth + 1;
        }

        for c in node.child_ids.iter().rev() {
            stack.push((c.clone(), next_depth));
        }
    }

    if out.is_empty() {
        out.push_str("(empty accessibility tree)\n");
    }
    (out, refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats_tree_and_assigns_refs() {
        let tree = json!({
            "nodes": [
                { "nodeId": "1", "role": {"value": "RootWebArea"}, "name": {"value": "Test"}, "childIds": ["2", "3"] },
                { "nodeId": "2", "role": {"value": "button"}, "name": {"value": "OK"}, "backendDOMNodeId": 42, "childIds": [], "parentId": "1" },
                { "nodeId": "3", "role": {"value": "textbox"}, "name": {"value": "Search"}, "backendDOMNodeId": 43, "childIds": [], "parentId": "1" }
            ]
        });
        let (text, refs) = format_ax_tree(&tree);
        assert!(text.contains("RootWebArea"));
        assert!(text.contains("button \"OK\" [ref=e1]"));
        assert!(text.contains("textbox \"Search\" [ref=e2]"));
        assert_eq!(refs.map.get("e1"), Some(&42));
        assert_eq!(refs.map.get("e2"), Some(&43));
    }

    #[test]
    fn handles_empty_tree() {
        let (text, refs) = format_ax_tree(&json!({ "nodes": [] }));
        assert!(text.contains("empty"));
        assert!(refs.map.is_empty());
    }
}
