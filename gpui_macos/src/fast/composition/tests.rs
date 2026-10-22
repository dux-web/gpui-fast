// Modified by Longbridge for gpui-fast.
use super::{OVERLAY_INPUT_IVAR, overlay_hit_test};
use cocoa::{
    appkit::NSView,
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize},
};
use gpui::{Bounds, ContentMask, Hitbox, HitboxBehavior, HitboxId, point, px, size};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel, YES},
    sel, sel_impl,
};
use std::{cell::RefCell, ffi::c_void, sync::OnceLock};

// No window, renderer or application is created: exercise AppKit's real view
// traversal using the same hitTest implementation as a composition surface.
struct Views {
    pool: id,
    parent: id,
    overlay: id,
    child: id,
    regions: Box<RefCell<Vec<Hitbox>>>,
}

impl Views {
    fn new() -> Self {
        static CLASS: OnceLock<usize> = OnceLock::new();
        let class = *CLASS.get_or_init(|| {
            let mut class = ClassDecl::new("GPUICompositionHitTest", class!(NSView)).unwrap();
            class.add_ivar::<*mut c_void>(OVERLAY_INPUT_IVAR);
            unsafe {
                class.add_method(
                    sel!(hitTest:),
                    overlay_hit_test as extern "C" fn(&Object, Sel, NSPoint) -> id,
                );
            }
            class.register() as *const Class as usize
        }) as *const Class;
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let parent = NSView::initWithFrame_(
                NSView::alloc(nil),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(400., 300.)),
            );
            let overlay: id = msg_send![class, alloc];
            let overlay = NSView::initWithFrame_(
                overlay,
                NSRect::new(NSPoint::new(40., 30.), NSSize::new(200., 160.)),
            );
            let child = NSView::initWithFrame_(
                NSView::alloc(nil),
                NSRect::new(NSPoint::new(20., 20.), NSSize::new(40., 40.)),
            );
            parent.addSubview_(overlay);
            overlay.addSubview_(child);
            let regions = Box::new(RefCell::new(Vec::new()));
            (*overlay).set_ivar(
                OVERLAY_INPUT_IVAR,
                (&*regions as *const RefCell<Vec<Hitbox>>) as *mut c_void,
            );
            Self {
                pool,
                parent,
                overlay,
                child,
                regions,
            }
        }
    }

    fn hit(&self, x: f64, y: f64) -> id {
        unsafe { msg_send![self.overlay, hitTest: NSPoint::new(x, y)] }
    }

    fn region(&self) {
        self.regions.borrow_mut().push(Hitbox {
            id: HitboxId::placeholder(),
            bounds: Bounds::new(point(px(100.), px(20.)), size(px(30.), px(30.))),
            content_mask: ContentMask::default(),
            behavior: HitboxBehavior::Normal,
        });
    }
}

impl Drop for Views {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.child, release];
            let _: () = msg_send![self.overlay, release];
            let _: () = msg_send![self.parent, release];
            self.pool.drain();
        }
    }
}

#[test]
fn nested_native_view_remains_interactive() {
    let views = Views::new();
    // The native descendant owns this point even with no GPUI hitbox.
    assert_eq!(views.hit(70., 60.), views.child);
    assert_eq!(views.hit(140., 60.), nil);
}

#[test]
fn hidden_overlay_does_not_take_input() {
    let views = Views::new();
    views.region();
    unsafe {
        let _: () = msg_send![views.overlay, setHidden: YES];
    }
    assert_eq!(views.hit(150., 160.), nil);
}

#[test]
fn regions_use_superview_coordinates() {
    let views = Views::new();
    views.region();
    assert_eq!(views.hit(150., 160.), views.overlay);
    assert_eq!(views.hit(135., 160.), nil);
    assert_eq!(views.hit(150., 130.), nil);
    // A scaled bounds rect changes the frame-to-local conversion.
    unsafe {
        let _: () = msg_send![views.overlay, setBoundsSize: NSSize::new(400., 320.)];
    }
    assert_eq!(views.hit(95., 175.), views.overlay);
    assert_eq!(views.hit(85., 175.), nil);
}
