// Copyright 2025 The AccessKit Authors.
//
// iOS adapter for exposing an AccessKit tree via UIAccessibilityContainer
// by setting UIView.accessibilityElements.

#![deny(unsafe_op_in_unsafe_fn)]

mod context;
mod filters;
mod node;
mod util;

mod adapter;
mod event;
pub use adapter::Adapter;

// Re-exports often used by host projects.
pub use objc2_foundation::{NSArray, NSObject};
pub use objc2_ui_kit::UIView;
