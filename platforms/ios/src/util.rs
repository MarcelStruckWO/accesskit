// Copyright 2025 The AccessKit Authors.

use accesskit::Rect;
use accesskit_consumer::Node;

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_ui_kit::UIView;

/// Convert an AccessKit rect (physical pixels) to a CGRect in the view's
/// container-space coordinate system (points). UIKit y-axis grows downward
/// by default, which matches the AccessKit coordinate convention used on
/// other platforms with a flipped view (so no extra flip needed).
pub(crate) fn to_cg_rect(_view: &UIView, rect: Rect) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: rect.x0,
            y: rect.y0,
        },
        size: CGSize {
            width: rect.width(),
            height: rect.height(),
        },
    }
}

/// Compute the element frame in container space for a node.
/// Falls back to zero rect if node has no bounding box.
pub(crate) fn element_frame_in_container(view: &UIView, node: &Node) -> Option<CGRect> {
    if let Some(bb) = node.bounding_box() {
        Some(to_cg_rect(view, bb))
    } else {
        None
    }
}
