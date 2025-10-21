// Copyright 2025 The AccessKit Authors.

use accesskit::Rect;
use accesskit_consumer::Node;

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_ui_kit::UIView;

/// Convert an AccessKit rect (physical pixels) to a CGRect in the view's
/// container-space coordinate system (points). UIKit y-axis grows downward
/// by default, which matches the AccessKit coordinate convention used on
/// other platforms with a flipped view (so no extra flip needed).
pub(crate) fn to_cg_rect(view: &UIView, rect: Rect) -> CGRect {
    println!("content scale factor {:?}", view.contentScaleFactor());
    // let scale = view.contentScaleFactor();
    let scale = 1.;
    CGRect {
        origin: CGPoint {
            x: rect.x0 / scale,
            y: rect.y0 / scale,
        },
        size: CGSize {
            width: rect.width() / scale,
            height: rect.height() / scale,
        },
    }
}

/// Compute the element frame in container space for a node.
/// Falls back to zero rect if node has no bounding box.
pub(crate) fn element_frame_in_container(view: &UIView, node: &Node) -> Option<CGRect> {
    println!(
        "bounding box for accesskit node {:?}: {:?}, {:?}, {:?}",
        node.label(),
        node.bounding_box(),
        node.has_bounds(),
        node.raw_bounds(),
    );
    if let Some(bb) = node.bounding_box() {
        Some(to_cg_rect(view, bb))
    } else {
        None
        // CGRect {
        //     origin: CGPoint { x: 0.0, y: 0.0 },
        //     size: CGSize {
        //         width: 100.0,
        //         height: 200.0,
        //     },
        // }
    }
}
