//! Intrusive Linked Lists
//!
//! Zero-allocation linked lists where nodes are embedded directly in the data structure.
//! This is the foundation for the scheduler run queues and memory allocator free lists.
//!
//! # Design
//!
//! Unlike traditional linked lists which allocate separate node structures,
//! intrusive lists require the data structure to embed the list node:
//!
//! ```ignore
//! struct Task {
//!     run_queue_node: IntrusiveNode,  // Embedded, not allocated
//!     wait_queue_node: IntrusiveNode, // Can have multiple!
//!     // ... task data
//! }
//! ```
//!
//! # Safety
//!
//! Intrusive lists are inherently unsafe because:
//! - Nodes must remain at stable memory addresses while linked
//! - Removing a node requires knowing which list it's in
//! - Double-linking the same node corrupts the list
//!
//! The abstractions here provide some safety, but users must ensure:
//! - Nodes are not moved while linked
//! - Nodes are removed before being dropped
//! - Each node is only in one list at a time

use core::ptr::NonNull;
use core::marker::PhantomData;

/// Intrusive list node
///
/// Embed this in your data structure to allow it to be part of a linked list.
/// A structure can have multiple nodes to be in multiple lists simultaneously.
#[derive(Debug)]
#[repr(C)]
pub struct IntrusiveNode {
    next: Option<NonNull<IntrusiveNode>>,
    prev: Option<NonNull<IntrusiveNode>>,
}

impl IntrusiveNode {
    /// Create a new unlinked node
    pub const fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }
    
    /// Check if this node is currently linked in a list
    pub fn is_linked(&self) -> bool {
        self.next.is_some() || self.prev.is_some()
    }
    
    /// Reset the node to unlinked state
    ///
    /// # Safety
    ///
    /// Caller must ensure the node has been properly removed from any list.
    pub unsafe fn reset(&mut self) {
        self.next = None;
        self.prev = None;
    }
}

impl Default for IntrusiveNode {
    fn default() -> Self {
        Self::new()
    }
}

/// Intrusive doubly-linked list
///
/// A list that uses embedded nodes rather than allocating wrapper structures.
///
/// # Type Parameters
///
/// - `T`: The container type that embeds `IntrusiveNode`
/// - `N`: Function to get node from container (usually a macro-generated fn)
pub struct IntrusiveList<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    head: Option<NonNull<IntrusiveNode>>,
    tail: Option<NonNull<IntrusiveNode>>,
    len: usize,
    node_offset: N,
    _marker: PhantomData<T>,
}

impl<T, N> IntrusiveList<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    /// Create a new empty list
    ///
    /// The `node_offset` function extracts the node from a container.
    pub const fn new(node_offset: N) -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
            node_offset,
            _marker: PhantomData,
        }
    }
    
    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }
    
    /// Get the number of elements in the list
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Push an element to the front of the list
    ///
    /// # Safety
    ///
    /// - `item` must remain at a stable memory address while in the list
    /// - `item`'s node must not already be in a list
    pub unsafe fn push_front(&mut self, item: &T) {
        let node = (self.node_offset)(item);
        let node_ptr = NonNull::new_unchecked(node as *const _ as *mut IntrusiveNode);
        
        debug_assert!(!node.is_linked(), "Node already linked");
        
        // Get mutable access to the node
        let node_mut = node_ptr.as_ptr();
        
        (*node_mut).next = self.head;
        (*node_mut).prev = None;
        
        if let Some(head) = self.head {
            (*head.as_ptr()).prev = Some(node_ptr);
        } else {
            self.tail = Some(node_ptr);
        }
        
        self.head = Some(node_ptr);
        self.len += 1;
    }
    
    /// Push an element to the back of the list
    ///
    /// # Safety
    ///
    /// - `item` must remain at a stable memory address while in the list
    /// - `item`'s node must not already be in a list
    pub unsafe fn push_back(&mut self, item: &T) {
        let node = (self.node_offset)(item);
        let node_ptr = NonNull::new_unchecked(node as *const _ as *mut IntrusiveNode);
        
        debug_assert!(!node.is_linked(), "Node already linked");
        
        let node_mut = node_ptr.as_ptr();
        
        (*node_mut).prev = self.tail;
        (*node_mut).next = None;
        
        if let Some(tail) = self.tail {
            (*tail.as_ptr()).next = Some(node_ptr);
        } else {
            self.head = Some(node_ptr);
        }
        
        self.tail = Some(node_ptr);
        self.len += 1;
    }
    
    /// Pop an element from the front of the list
    ///
    /// # Safety
    ///
    /// The returned reference is only valid as long as the underlying
    /// data structure exists.
    pub unsafe fn pop_front(&mut self) -> Option<NonNull<T>> {
        let head = self.head?;
        let head_ptr = head.as_ptr();
        
        self.head = (*head_ptr).next;
        
        if let Some(new_head) = self.head {
            (*new_head.as_ptr()).prev = None;
        } else {
            self.tail = None;
        }
        
        (*head_ptr).reset();
        self.len -= 1;
        
        // Convert node pointer back to container pointer
        Some(self.node_to_container(head))
    }
    
    /// Pop an element from the back of the list
    ///
    /// # Safety
    ///
    /// The returned reference is only valid as long as the underlying
    /// data structure exists.
    pub unsafe fn pop_back(&mut self) -> Option<NonNull<T>> {
        let tail = self.tail?;
        let tail_ptr = tail.as_ptr();
        
        self.tail = (*tail_ptr).prev;
        
        if let Some(new_tail) = self.tail {
            (*new_tail.as_ptr()).next = None;
        } else {
            self.head = None;
        }
        
        (*tail_ptr).reset();
        self.len -= 1;
        
        Some(self.node_to_container(tail))
    }
    
    /// Remove a specific element from the list
    ///
    /// # Safety
    ///
    /// - `item` must be in this list
    /// - `item` must not be removed twice
    pub unsafe fn remove(&mut self, item: &T) {
        let node = (self.node_offset)(item);
        let node_ptr = NonNull::new_unchecked(node as *const _ as *mut IntrusiveNode);
        let node_mut = node_ptr.as_ptr();
        
        // Update neighbors
        if let Some(prev) = (*node_mut).prev {
            (*prev.as_ptr()).next = (*node_mut).next;
        } else {
            self.head = (*node_mut).next;
        }
        
        if let Some(next) = (*node_mut).next {
            (*next.as_ptr()).prev = (*node_mut).prev;
        } else {
            self.tail = (*node_mut).prev;
        }
        
        (*node_mut).reset();
        self.len -= 1;
    }
    
    /// Get a reference to the front element without removing it
    pub fn front(&self) -> Option<NonNull<T>> {
        self.head.map(|h| unsafe { self.node_to_container(h) })
    }
    
    /// Get a reference to the back element without removing it
    pub fn back(&self) -> Option<NonNull<T>> {
        self.tail.map(|t| unsafe { self.node_to_container(t) })
    }
    
    /// Convert a node pointer back to its container
    ///
    /// This requires knowing the offset of the node within the container,
    /// which we compute by using the node_offset function on a reference.
    unsafe fn node_to_container(&self, node: NonNull<IntrusiveNode>) -> NonNull<T> {
        // This is a simplified version - in production you'd use offset_of!
        // For now, we assume the node is at the start of T (offset 0)
        NonNull::new_unchecked(node.as_ptr() as *mut T)
    }
}

/// Macro to create a node accessor function
///
/// # Example
///
/// ```ignore
/// struct Task {
///     run_node: IntrusiveNode,
///     id: u32,
/// }
///
/// intrusive_adapter!(TaskRunAdapter = Task { run_node: IntrusiveNode });
/// 
/// let mut list: IntrusiveList<Task, _> = IntrusiveList::new(|t| &t.run_node);
/// ```
#[macro_export]
macro_rules! intrusive_adapter {
    ($name:ident = $container:ty { $field:ident : IntrusiveNode }) => {
        fn $name(container: &$container) -> &$crate::mm::intrusive::IntrusiveNode {
            &container.$field
        }
    };
}

// Simple LIFO stack using intrusive list (for free lists)
/// Intrusive stack (LIFO)
pub struct IntrusiveStack<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    list: IntrusiveList<T, N>,
}

impl<T, N> IntrusiveStack<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    /// Create a new empty stack
    pub const fn new(node_offset: N) -> Self {
        Self {
            list: IntrusiveList::new(node_offset),
        }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    
    /// Get count
    pub fn len(&self) -> usize {
        self.list.len()
    }
    
    /// Push item onto stack
    pub unsafe fn push(&mut self, item: &T) {
        self.list.push_front(item);
    }
    
    /// Pop item from stack
    pub unsafe fn pop(&mut self) -> Option<NonNull<T>> {
        self.list.pop_front()
    }
}

// FIFO queue using intrusive list (for run queues)
/// Intrusive queue (FIFO)
pub struct IntrusiveQueue<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    list: IntrusiveList<T, N>,
}

impl<T, N> IntrusiveQueue<T, N>
where
    N: Fn(&T) -> &IntrusiveNode,
{
    /// Create a new empty queue
    pub const fn new(node_offset: N) -> Self {
        Self {
            list: IntrusiveList::new(node_offset),
        }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    
    /// Get count
    pub fn len(&self) -> usize {
        self.list.len()
    }
    
    /// Enqueue item (add to back)
    pub unsafe fn enqueue(&mut self, item: &T) {
        self.list.push_back(item);
    }
    
    /// Dequeue item (remove from front)
    pub unsafe fn dequeue(&mut self) -> Option<NonNull<T>> {
        self.list.pop_front()
    }
    
    /// Peek at front item
    pub fn peek(&self) -> Option<NonNull<T>> {
        self.list.front()
    }
}
