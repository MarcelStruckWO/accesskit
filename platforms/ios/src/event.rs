use std::collections::VecDeque;
use std::rc::Rc;

use accesskit::{NodeId, Role};
use accesskit_consumer::{FilterResult, Node, TreeChangeHandler};
use hashbrown::HashSet;
use objc2_ui_kit::{UIAccessibilityLayoutChangedNotification, UIAccessibilityNotifications};

use crate::context::Context;
use crate::filters::filter;
use crate::node::{NodeWrapper, Value};

// pub(crate) fn focus_event(node_id: NodeId) -> QueuedEvent {
//     QueuedEvent::Generic {
//         node_id,
//         notification: unsafe { UIAccessibilityElementFocusedNotification },
//     }
// }

pub(crate) struct EventGenerator {
    context: Rc<Context>,
    events: Vec<QueuedEvent>,
    text_changed: HashSet<NodeId>,
    selected_rows_changed: HashSet<NodeId>,
}

impl EventGenerator {
    pub(crate) fn new(context: Rc<Context>) -> Self {
        Self {
            context,
            events: Vec::new(),
            text_changed: HashSet::new(),
            selected_rows_changed: HashSet::new(),
        }
    }

    fn insert_text_change_if_needed(&mut self, node: &Node) {
        if node.role() != Role::TextRun {
            return;
        }
        // if let Some(node) = node.filtered_parent(&filter) {
        //     // todo
        //     // self.insert_text_change_if_needed_parent(node);
        // }
    }

    fn remove_subtree(&mut self, node: &Node) {
        let mut to_remove = VecDeque::new();
        to_remove.push_back(*node);

        while let Some(node) = to_remove.pop_front() {
            for child in node.filtered_children(&filter) {
                to_remove.push_back(child);
            }

            self.events.push(QueuedEvent::NodeDestroyed(node.id()));
        }
    }

    // fn enqueue_selected_rows_change_if_needed_parent(&mut self, node: Node) {
    //     let id = node.id();
    //     if self.selected_rows_changed.contains(&id) {
    //         return;
    //     }
    //     self.events.push(QueuedEvent::Generic {
    //         node_id: id,
    //         notification: unsafe { UI },
    //     });
    //     self.selected_rows_changed.insert(id);
    // }

    // fn enqueue_selected_rows_change_if_needed(&mut self, node: &Node) {
    //     let wrapper = NodeWrapper(node);
    //     if !wrapper.is_item_like() {
    //         return;
    //     }
    //     if let Some(node) = node.selection_container(&filter) {
    //         self.enqueue_selected_rows_change_if_needed_parent(node);
    //     }
    // }
}

impl TreeChangeHandler for EventGenerator {
    fn node_added(&mut self, node: &accesskit_consumer::Node) {
        self.insert_text_change_if_needed(node);
        if filter(node) != FilterResult::Include {
            return;
        }
        // if let Some(true) = node.is_selected() {
        //     self.enqueue_selected_rows_change_if_needed(node);
        // }
        // if node.value().is_some() && node.live() != Live::Off {
        //     self.events
        //         .push(QueuedEvent::live_region_announcement(node));
        // }
    }

    fn node_updated(
        &mut self,
        old_node: &accesskit_consumer::Node,
        new_node: &accesskit_consumer::Node,
    ) {
        if old_node.raw_value() != new_node.raw_value() {
            self.insert_text_change_if_needed(new_node);
        }
        let old_filter_result = filter(old_node);
        let new_filter_result = filter(new_node);
        if new_filter_result != FilterResult::Include {
            if old_filter_result == FilterResult::Include && old_node.is_selected() == Some(true) {
                // self.enqueue_selected_rows_change_if_needed(old_node);
            }
            if new_filter_result == FilterResult::ExcludeSubtree {
                self.remove_subtree(old_node);
            } else {
                self.events.push(QueuedEvent::NodeDestroyed(new_node.id()));
            }
            return;
        }
        let node_id = new_node.id();
        let old_wrapper = NodeWrapper(old_node);
        let new_wrapper = NodeWrapper(new_node);
        if old_wrapper.title() != new_wrapper.title() {
            self.events.push(QueuedEvent::Generic {
                node_id,
                notification: unsafe { UIAccessibilityLayoutChangedNotification },
            });
        }
        let new_value = new_wrapper.value();
        if old_wrapper.value() != new_value {
            if !new_node.is_focused() && new_value.is_some_and(|v| matches!(v, Value::Bool(_))) {
                // Bool value changed event for the focused node must come last
                // in order for VoiceOver to announce it. Otherwise, if we raise
                // bool value changed events for other nodes after this one, VoiceOver
                // will announce them instead.
                self.events.insert(
                    0,
                    QueuedEvent::Generic {
                        node_id,
                        notification: unsafe { UIAccessibilityLayoutChangedNotification },
                    },
                );
            } else {
                self.events.push(QueuedEvent::Generic {
                    node_id,
                    notification: unsafe { UIAccessibilityLayoutChangedNotification },
                });
            }
        }
        if old_wrapper.supports_text_ranges()
            && new_wrapper.supports_text_ranges()
            && old_wrapper.raw_text_selection() != new_wrapper.raw_text_selection()
        {
            self.events.push(QueuedEvent::Generic {
                node_id,
                notification: unsafe { UIAccessibilityLayoutChangedNotification },
            });
        }
        // if new_node.value().is_some()
        //     && new_node.live() != Live::Off
        //     && (new_node.value() != old_node.value()
        //         || new_node.live() != old_node.live()
        //         || old_filter_result != FilterResult::Include)
        // {
        //     // self.events
        //     //     .push(QueuedEvent::live_region_announcement(new_node));
        // }
        if new_node.is_selected() != old_node.is_selected()
            || (old_filter_result != FilterResult::Include && new_node.is_selected() == Some(true))
        {
            // self.enqueue_selected_rows_change_if_needed(new_node);
        }
    }

    fn focus_moved(
        &mut self,
        old_node: Option<&accesskit_consumer::Node>,
        new_node: Option<&accesskit_consumer::Node>,
    ) {
        if let Some(new_node) = new_node {
            if filter(new_node) != FilterResult::Include {
                return;
            }
            // self.events.push(focus_event(new_node.id()));
        }
    }

    fn node_removed(&mut self, node: &accesskit_consumer::Node) {
        // todo!()
    }
}

#[derive(Debug)]
pub(crate) enum QueuedEvent {
    Generic {
        node_id: NodeId,
        notification: UIAccessibilityNotifications,
    },
    NodeDestroyed(NodeId),
}

impl QueuedEvent {
    fn raise(self, context: &Rc<Context>) {
        // println!("got event for accesskit: {self:?}");
        // match self {
        //     QueuedEvent::Generic {
        //         node_id,
        //         notification,
        //     } => {
        //         if let Some(element) = context.get_or_create_node(node_id) {
        //             unsafe {
        //                 UIAccessibilityPostNotification(notification, Some(&*element));
        //             }
        //         }
        //     }
        //     QueuedEvent::NodeDestroyed(node_id) => {
        //         context.remove_accessibility_element(node_id);
        //     }
        // }
    }
}

/// Events generated by a tree update.
#[must_use = "events must be explicitly raised"]
pub struct QueuedEvents {
    context: Rc<Context>,
    events: Vec<QueuedEvent>,
}

impl QueuedEvents {
    pub(crate) fn new(context: Rc<Context>, events: Vec<QueuedEvent>) -> Self {
        Self { context, events }
    }

    /// Raise all queued events synchronously.
    ///
    /// It is unknown whether accessibility methods on the view may be
    /// called while events are being raised. This means that any locks
    /// or runtime borrows required to access the adapter must not
    /// be held while this method is called.
    pub fn raise(self) {
        for event in self.events {
            event.raise(&self.context);
        }
    }
}
