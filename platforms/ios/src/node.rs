// Copyright 2025 The AccessKit Authors.

use accesskit::{Role, TextSelection, Toggled};
use accesskit_consumer::Node;
use objc2::rc::Id;
use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{NSObjectUIAccessibilityContainer, UIAccessibilityElement, UIView};
use std::rc::Rc;

use crate::context::Context;
use crate::filters::filter;
use crate::util::element_frame_in_container;

#[derive(PartialEq)]
pub(crate) enum Value {
    Bool(bool),
    Number(f64),
    String(String),
}

pub(crate) struct NodeWrapper<'a>(pub(crate) &'a Node<'a>);

impl NodeWrapper<'_> {
    fn is_root(&self) -> bool {
        self.0.is_root()
    }

    pub(crate) fn title(&self) -> Option<String> {
        if self.is_root() && self.0.role() == Role::Window {
            // If the group element that we expose for the top-level window
            // includes a title, VoiceOver behavior is broken.
            return None;
        }
        self.0.label()
    }

    pub(crate) fn description(&self) -> Option<String> {
        self.0.description()
    }

    pub(crate) fn placeholder(&self) -> Option<&str> {
        self.0.placeholder()
    }

    pub(crate) fn value(&self) -> Option<Value> {
        if let Some(toggled) = self.0.toggled() {
            return Some(Value::Bool(toggled != Toggled::False));
        }
        if self.0.role() == Role::Tab {
            // On Mac, tabs are exposed as radio buttons, and are treated as checkable.
            // Also, `Node::is_selected` is mapped to checked via `accessibilityValue`.
            return Some(Value::Bool(self.0.is_selected().unwrap_or(false)));
        }
        if let Some(value) = self.0.value() {
            return Some(Value::String(value));
        }
        if let Some(value) = self.0.numeric_value() {
            return Some(Value::Number(value));
        }
        None
    }

    pub(crate) fn supports_text_ranges(&self) -> bool {
        self.0.supports_text_ranges()
    }

    pub(crate) fn raw_text_selection(&self) -> Option<&TextSelection> {
        self.0.raw_text_selection()
    }

    fn is_container_with_selectable_children(&self) -> bool {
        self.0.is_container_with_selectable_children() && self.0.role() != Role::TabList
    }

    pub(crate) fn is_item_like(&self) -> bool {
        self.0.is_item_like() && self.0.role() != Role::Tab
    }
}

/// Simple wrapper for an element instance; keeps the Id type available for caching if needed.
#[derive(Debug)]
pub(crate) struct ElementRef {
    pub element: Id<UIAccessibilityElement>,
}

fn label_for(node: &Node) -> Option<String> {
    // Avoid title on root window-like nodes; for iOS, being conservative is fine.
    // node.label()
    node.value().or(node.label())
}

fn hint_for(node: &Node) -> Option<String> {
    node.description()
}

// Extremely minimal role mapping just to get reasonable default behavior.
// We can expand this mapping to set UIAccessibilityTraits later.
fn is_accessibility_element(node: &Node) -> bool {
    // Include only nodes that pass the common filter and are not Role::Window-like.
    filter(node).is_include()
}

fn string_value_for(node: &Node) -> Option<String> {
    if let Some(toggled) = node.toggled() {
        let on = toggled != Toggled::False;
        return Some(if on { "1" } else { "0" }.to_string());
    }
    if let Some(v) = node.value() {
        return Some(v);
    }
    if let Some(n) = node.numeric_value() {
        return Some(format!("{n}"));
    }
    None
}

/// Build a UIAccessibilityElement for a node and attach basic properties.
/// The element's frame is expressed in the container view's coordinate space.
pub(crate) fn build_element_for_node(
    context: &Rc<Context>,
    container_view: &UIView,
    node: &Node,
) -> Option<ElementRef> {
    if !is_accessibility_element(node) {
        return None;
    }

    let element: Retained<UIAccessibilityElement> = unsafe {
        let el = UIAccessibilityElement::alloc(context.mtm.clone());
        UIAccessibilityElement::initWithAccessibilityContainer(el, container_view)
    };

    let Some(frame) = element_frame_in_container(container_view, node) else {
        return None;
    };
    element.setAccessibilityFrameInContainerSpace(frame);

    if let Some(label) = label_for(node) {
        let s = NSString::from_str(&label);
        element.setAccessibilityLabel(Some(&s));
    }

    if let Some(hint) = hint_for(node) {
        let s = NSString::from_str(&hint);
        element.setAccessibilityHint(Some(&s));
    }

    if let Some(value_str) = string_value_for(node) {
        let s = NSString::from_str(&value_str);
        element.setAccessibilityValue(Some(&s));
    }

    // Traits mapping can be added later; by default, omitted traits behave as static text.
    // Example for expansion:
    // let traits: UIAccessibilityTraits = UIAccessibilityTraitNone;
    // if node.role() == Role::Button { traits |= UIAccessibilityTraitButton; }
    // unsafe { let _: () = msg_send_id![&*element, setAccessibilityTraits: traits]; }

    let children = elements_for_children(context, container_view, node.filtered_children(filter));
    let accessibility_elements = children.downcast().unwrap();
    unsafe { element.setAccessibilityElements(Some(&accessibility_elements), context.mtm) };

    Some(ElementRef { element })
}

pub(crate) fn elements_for_children<'a>(
    context: &Rc<Context>,
    container_view: &UIView,
    children: impl Iterator<Item = Node<'a>>,
) -> Retained<NSArray<UIAccessibilityElement>> {
    let mut out: Vec<Retained<UIAccessibilityElement>> = Vec::new();
    for child in children {
        if let Some(el) = build_element_for_node(context, container_view, &child) {
            out.push(el.element);
        }
    }
    NSArray::from_retained_slice(&out)
}

/// Determine which nodes to expose as top-level elements of the container:
/// - If the root is included, expose just the root.
/// - Otherwise, expose included children (filtered).
pub(crate) fn elements_for_root(
    context: &Rc<Context>,
    container_view: &UIView,
    root: Node<'_>,
) -> Id<NSArray<UIAccessibilityElement>> {
    let top_level_elements = if filter(&root).is_include() && root.role() != Role::Window {
        let mut v = Vec::new();
        if let Some(el) = build_element_for_node(context, container_view, &root) {
            v.push(el.element);
        }
        v
    } else {
        root.filtered_children(filter)
            .filter_map(|child| {
                build_element_for_node(context, container_view, &child).map(|e| e.element)
            })
            .collect::<Vec<_>>()
    };
    NSArray::from_retained_slice(&top_level_elements)
}

trait FilterFlag {
    fn is_include(&self) -> bool;
}
impl FilterFlag for accesskit_consumer::FilterResult {
    fn is_include(&self) -> bool {
        matches!(self, accesskit_consumer::FilterResult::Include)
    }
}
