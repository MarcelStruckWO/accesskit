// Copyright 2025 The AccessKit Authors.

use crate::{
    context::{ActionHandlerNoMut, ActionHandlerWrapper, Context},
    event::EventGenerator,
    node::elements_for_root,
};
use accesskit::{
    ActionHandler, ActivationHandler, Node as NodeProvider, NodeId, Role, Tree as TreeData,
    TreeUpdate,
};
use accesskit_consumer::Tree;
use objc2::{
    rc::{Retained, Weak},
    runtime::AnyObject,
};
use objc2_foundation::{MainThreadMarker, NSArray};
use objc2_ui_kit::{
    NSObjectUIAccessibilityContainer, UIAccessibilityLayoutChangedNotification,
    UIAccessibilityPostNotification, UIView,
};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

const PLACEHOLDER_ROOT_ID: NodeId = NodeId(0);

enum State {
    Inactive {
        view: Weak<UIView>,
        // view: *mut UIView,
        is_view_focused: bool,
        action_handler: Rc<dyn ActionHandlerNoMut>,
        mtm: MainThreadMarker,
    },
    Placeholder {
        placeholder_context: Rc<Context>,
        is_view_focused: bool,
        action_handler: Rc<dyn ActionHandlerNoMut>,
    },
    Active(Rc<Context>),
}

impl Debug for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Inactive {
                view,
                is_view_focused,
                action_handler: _,
                mtm,
            } => f
                .debug_struct("Inactive")
                .field("view", view)
                .field("is_view_focused", is_view_focused)
                .field("mtm", mtm)
                .finish(),
            State::Placeholder {
                placeholder_context,
                is_view_focused,
                action_handler: _,
            } => f
                .debug_struct("Placeholder")
                .field("placeholder_context", placeholder_context)
                .field("is_view_focused", is_view_focused)
                .finish(),
            State::Active(context) => f.debug_struct("Active").field("context", context).finish(),
        }
    }
}

struct PlaceholderActionHandler;
impl ActionHandler for PlaceholderActionHandler {
    fn do_action(&mut self, _request: accesskit::ActionRequest) {}
}

#[derive(Debug)]
pub struct Adapter {
    state: State,
}

impl Adapter {
    /// Create a new iOS adapter. Must be called on the main thread.
    ///
    /// # Safety
    /// `view` must be a valid, unreleased pointer to a UIView.
    pub unsafe fn new(
        view: *mut UIView,
        is_view_focused: bool,
        action_handler: impl 'static + ActionHandler,
    ) -> Self {
        let view = Weak::new(unsafe { &*view });
        let mtm = MainThreadMarker::new().unwrap();
        let state = State::Inactive {
            view,
            is_view_focused,
            action_handler: Rc::new(ActionHandlerWrapper::new(action_handler)),
            mtm,
        };
        Self { state }
    }

    fn set_accessibility_elements_on_view(context: &Rc<Context>) {
        let view = match context.view.load() {
            Some(v) => v,
            None => return,
        };
        let tree = context.tree.borrow();
        let state = tree.state();
        let root = state.root();

        // // pretty print debug info for tree
        // println!("accesskit tree state:");
        // fn print_node(context: &Rc<Context>, node: &Node<'_>, indent: usize) {
        //     print!("[accesskit tree] ");
        //     for _ in 0..indent {
        //         print!("  ");
        //     }
        //     println!(
        //         "- id: {:?}, role: {:?}, name: {:?}, value: {:?}, bounds: {:?}",
        //         node.id(),
        //         node.role(),
        //         node.label().map(|n| n.to_string()),
        //         node.value(),
        //         node.bounding_box(),
        //     );
        //     for child in node.children() {
        //         print_node(context, &child, indent + 1);
        //     }
        // }
        // print_node(context, &root, 0);

        let elements: Retained<NSArray<AnyObject>> = {
            let arr = elements_for_root(&context, &view, root);

            let mut v: Vec<Retained<AnyObject>> = Vec::new();
            for i in 0..arr.len() {
                let obj = arr.objectAtIndex(i).into();
                v.push(obj);
            }
            NSArray::from_retained_slice(&v)
        };

        unsafe {
            view.setAccessibilityElements(Some(&*elements), context.mtm);
        }

        unsafe {
            UIAccessibilityPostNotification(UIAccessibilityLayoutChangedNotification, None);
        }
    }

    /// If and only if the tree has been initialized, call the provided function
    /// and apply the resulting update. Note: If the caller's implementation of
    /// ActivationHandler::request_initial_tree initially returned None,
    /// the TreeUpdate returned by the provided function must contain a full tree.
    pub fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
        match &mut self.state {
            State::Inactive { .. } => {}
            State::Placeholder {
                placeholder_context,
                is_view_focused,
                action_handler,
            } => {
                let tree = Tree::new(update_factory(), true);
                let context = Context::new(
                    placeholder_context.view.clone(),
                    tree,
                    Rc::clone(action_handler),
                    placeholder_context.mtm,
                );
                // let result = context
                //     .tree
                //     .borrow()
                //     .state()
                //     .focus_id()
                //     .map(|id| QueuedEvents::new(Rc::clone(&context), vec![focus_event(id)]));
                Self::set_accessibility_elements_on_view(&context);
                self.state = State::Active(context);
            }
            State::Active(context) => {
                {
                    let mut event_generator = EventGenerator::new(context.clone());
                    let mut tree = context.tree.borrow_mut();
                    tree.update_and_process_changes(update_factory(), &mut event_generator);
                }
                Self::set_accessibility_elements_on_view(context);
            }
        }
    }

    /// Update the tree state based on whether the view is focused; for iOS, we
    /// recompute the elements but do not raise events here.
    pub fn update_view_focus_state(&mut self, is_focused: bool) {
        match &mut self.state {
            State::Inactive {
                is_view_focused, ..
            } => *is_view_focused = is_focused,
            State::Placeholder {
                is_view_focused, ..
            } => *is_view_focused = is_focused,
            State::Active(context) => {
                {
                    let mut event_generator = EventGenerator::new(context.clone());
                    let mut tree = context.tree.borrow_mut();
                    tree.update_host_focus_state_and_process_changes(
                        is_focused,
                        &mut event_generator,
                    );
                }
                Self::set_accessibility_elements_on_view(context);
            }
        }
    }

    fn get_or_init_context<H: ActivationHandler + ?Sized>(
        &mut self,
        activation_handler: &mut H,
    ) -> Rc<Context> {
        match &self.state {
            State::Inactive {
                view,
                is_view_focused,
                action_handler,
                mtm,
            } => match activation_handler.request_initial_tree() {
                Some(initial_state) => {
                    let tree = Tree::new(initial_state, true);
                    let context = Context::new(view.clone(), tree, Rc::clone(action_handler), *mtm);
                    Self::set_accessibility_elements_on_view(&context);
                    let result = Rc::clone(&context);
                    self.state = State::Active(context);
                    result
                }
                None => {
                    let placeholder_update = TreeUpdate {
                        nodes: vec![(PLACEHOLDER_ROOT_ID, NodeProvider::new(Role::Window))],
                        tree: Some(TreeData::new(PLACEHOLDER_ROOT_ID)),
                        focus: PLACEHOLDER_ROOT_ID,
                    };

                    let placeholder_tree = Tree::new(placeholder_update, false);
                    let placeholder_context = Context::new(
                        view.clone(),
                        placeholder_tree,
                        Rc::new(ActionHandlerWrapper::new(PlaceholderActionHandler {})),
                        *mtm,
                    );
                    let result = Rc::clone(&placeholder_context);
                    self.state = State::Placeholder {
                        placeholder_context,
                        is_view_focused: *is_view_focused,
                        action_handler: Rc::clone(action_handler),
                    };
                    result
                }
            },
            State::Placeholder {
                placeholder_context,
                ..
            } => Rc::clone(placeholder_context),
            State::Active(context) => Rc::clone(context),
        }
    }

    /// Initialize the adapter with the provided activation handler, and
    /// populate the UIView.accessibilityElements.
    ///
    /// Call this once after constructing the adapter, passing your activation handler.
    pub fn activate<H: ActivationHandler + ?Sized>(&mut self, activation_handler: &mut H) {
        let context = self.get_or_init_context(activation_handler);
        Self::set_accessibility_elements_on_view(&context);
    }
}
