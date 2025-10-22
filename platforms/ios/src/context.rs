// Copyright 2025 The AccessKit Authors.

use accesskit::{ActionHandler, ActionRequest, NodeId};
use accesskit_consumer::Tree;
use hashbrown::HashMap;
use objc2::rc::WeakId;
use objc2_foundation::MainThreadMarker;
use objc2_ui_kit::UIView;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::{Rc, Weak};

use crate::node::ElementRef;

pub(crate) trait ActionHandlerNoMut {
    fn do_action(&self, request: ActionRequest);
}

pub(crate) struct ActionHandlerWrapper<H: ActionHandler>(RefCell<H>);

impl<H: 'static + ActionHandler> ActionHandlerWrapper<H> {
    pub(crate) fn new(inner: H) -> Self {
        Self(RefCell::new(inner))
    }
}

impl<H: ActionHandler> ActionHandlerNoMut for ActionHandlerWrapper<H> {
    fn do_action(&self, request: ActionRequest) {
        self.0.borrow_mut().do_action(request)
    }
}

pub(crate) struct Context {
    pub(crate) view: WeakId<UIView>,
    pub(crate) tree: RefCell<Tree>,
    pub(crate) action_handler: Rc<dyn ActionHandlerNoMut>,
    // Optional cache if we want to reuse elements by node id in future iterations.
    pub(crate) elements_by_id: RefCell<HashMap<NodeId, Weak<ElementRef>>>,
    pub(crate) mtm: MainThreadMarker,
}

impl Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("view", &self.view)
            .field("tree", &self.tree)
            .field("action_handler", &"ActionHandler")
            .field("elements_by_id", &"...")
            .field("mtm", &self.mtm)
            .finish()
    }
}

impl Context {
    pub(crate) fn new(
        view: WeakId<UIView>,
        tree: Tree,
        action_handler: Rc<dyn ActionHandlerNoMut>,
        mtm: MainThreadMarker,
    ) -> Rc<Self> {
        Rc::new(Self {
            view,
            tree: RefCell::new(tree),
            action_handler,
            elements_by_id: RefCell::new(HashMap::new()),
            mtm,
        })
    }

    pub(crate) fn do_action(&self, request: ActionRequest) {
        self.action_handler.do_action(request);
    }

    // pub(crate) fn get_or_create_node(self: &Rc<Self>, id: NodeId) -> Id<PlatformNode> {}
}
