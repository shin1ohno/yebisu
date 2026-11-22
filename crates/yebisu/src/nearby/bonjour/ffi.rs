//! Apple DNS-SD calls and callbacks are confined to a single worker thread.
//! Only DNSServiceProcessResult invokes callbacks, and it borrows the worker's
//! stable, boxed contexts until it returns. Drop invalidates each reference
//! before freeing its callback context, and children before the parent.
#![allow(unsafe_code)]

use super::{super::reply::resolved_device, Device, POLL};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    io,
    ptr::{self, NonNull},
    rc::Rc,
};

const ADD: u32 = 0x2;
const SHARE_CONNECTION: u32 = 0x4000;
const MAX_DNS_NAME: usize = 1005;
const MAX_EVENTS: usize = 128;
const POLL_IN: i16 = 0x0001;
pub(super) const POLICY_DENIED: i32 = -65570;

type Raw = *mut c_void;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Service {
    interface: u32,
    name: CString,
    domain: CString,
}

#[derive(Debug)]
pub(super) struct Error(pub i32);

pub(super) enum Event {
    Found(Service),
    Lost(Service),
    Resolved(Service, Device),
    ResolveFailed(Service),
    Error(Error),
}

#[derive(Default)]
struct Events {
    queue: Vec<Event>,
    overflow: bool,
}

type Inbox = Rc<RefCell<Events>>;

fn push(inbox: &RefCell<Events>, event: Event) {
    let mut inbox = inbox.borrow_mut();
    if inbox.queue.len() < MAX_EVENTS {
        inbox.queue.push(event);
    } else {
        inbox.overflow = true;
    }
}

struct Reference(NonNull<c_void>);

impl Reference {
    fn from_raw(raw: Raw) -> Result<Self, Error> {
        NonNull::new(raw).map(Self).ok_or(Error(-1))
    }

    fn raw(&self) -> Raw {
        self.0.as_ptr()
    }
}

impl Drop for Reference {
    fn drop(&mut self) {
        // SAFETY: each successfully created reference has a single owner.
        unsafe { DNSServiceRefDeallocate(self.raw()) }
    }
}

struct ResolveContext {
    inbox: Inbox,
    service: Service,
}

pub(super) struct Browser {
    queries: BTreeMap<Service, (Reference, Box<ResolveContext>)>,
    _browse: Reference,
    inbox: Inbox,
    connection: Reference,
}

impl Browser {
    pub(super) fn new() -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a writable reference slot for the parent connection.
        check(unsafe { DNSServiceCreateConnection(&mut raw) })?;
        let connection = Reference::from_raw(raw)?;
        let inbox = Rc::new(RefCell::new(Events::default()));
        raw = connection.raw();
        // SAFETY: the Rc's pointee stays alive until the browse child is dropped.
        // A shared child is returned in raw while the stored parent is intact.
        check(unsafe {
            DNSServiceBrowse(
                &mut raw,
                SHARE_CONNECTION,
                0,
                c"_ssh._tcp".as_ptr(),
                c"local.".as_ptr(),
                browse_reply,
                Rc::as_ptr(&inbox).cast_mut().cast(),
            )
        })?;
        Ok(Self {
            queries: BTreeMap::new(),
            _browse: Reference::from_raw(raw)?,
            inbox,
            connection,
        })
    }

    pub(super) fn resolve(&mut self, service: &Service) -> Result<bool, Error> {
        let mut context = Box::new(ResolveContext {
            inbox: Rc::clone(&self.inbox),
            service: service.clone(),
        });
        let mut raw = self.connection.raw();
        // SAFETY: service strings are NUL-terminated and the boxed callback
        // context outlives the child query. No thread shares these pointers.
        let code = unsafe {
            DNSServiceResolve(
                &mut raw,
                SHARE_CONNECTION,
                service.interface,
                service.name.as_ptr(),
                c"_ssh._tcp".as_ptr(),
                service.domain.as_ptr(),
                resolve_reply,
                ptr::from_mut(context.as_mut()).cast(),
            )
        };
        if code != 0 && code != POLICY_DENIED {
            // One disappearing service cannot stop the rest of the browse.
            return Ok(false);
        }
        check(code)?;
        self.queries
            .insert(service.clone(), (Reference::from_raw(raw)?, context));
        Ok(true)
    }

    pub(super) fn cancel_resolution(&mut self, service: &Service) {
        self.queries.remove(service);
    }

    pub(super) fn poll(&mut self) -> Result<Vec<Event>, Error> {
        // SAFETY: the live connection owns the fd and poll borrows it for one call.
        let fd = unsafe { DNSServiceRefSockFD(self.connection.raw()) };
        if fd < 0 {
            return Err(Error(fd));
        }
        let mut descriptor = PollFd {
            fd,
            events: POLL_IN,
            revents: 0,
        };
        let ready = unsafe { poll(&mut descriptor, 1, POLL.as_millis() as c_int) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(Vec::new());
            }
            return Err(Error(error.raw_os_error().unwrap_or(-1)));
        }
        if ready > 0 {
            if descriptor.revents & POLL_IN == 0 {
                return Err(Error(-1));
            }
            // SAFETY: poll observed readable bytes. Every child and its context
            // is still alive. No Rust borrow of the event queue spans callbacks.
            check(unsafe { DNSServiceProcessResult(self.connection.raw()) })?;
        }
        let mut inbox = self.inbox.borrow_mut();
        if inbox.overflow {
            return Err(Error(-1));
        }
        Ok(std::mem::take(&mut inbox.queue))
    }
}

fn check(code: i32) -> Result<(), Error> {
    if code == 0 { Ok(()) } else { Err(Error(code)) }
}

// SAFETY: a non-null pointer must be a C string valid for this callback.
unsafe fn dns_name(name: *const c_char) -> Option<CString> {
    if name.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name) };
    (name.to_bytes().len() <= MAX_DNS_NAME).then(|| name.to_owned())
}

extern "C" fn browse_reply(
    _: Raw,
    flags: u32,
    interface: u32,
    code: i32,
    name: *const c_char,
    _: *const c_char,
    domain: *const c_char,
    context: *mut c_void,
) {
    // SAFETY: DNS-SD returns the same live inbox context passed to Browse.
    let inbox = unsafe { &*context.cast::<RefCell<Events>>() };
    if code != 0 {
        push(inbox, Event::Error(Error(code)));
        return;
    }
    // SAFETY: successful DNS-SD callbacks supply valid strings until return.
    let Some((name, domain)) = (unsafe { dns_name(name).zip(dns_name(domain)) }) else {
        return;
    };
    let service = Service {
        interface,
        name,
        domain,
    };
    let event = if flags & ADD != 0 {
        Event::Found(service)
    } else {
        Event::Lost(service)
    };
    push(inbox, event);
}

extern "C" fn resolve_reply(
    _: Raw,
    _: u32,
    _: u32,
    code: i32,
    _: *const c_char,
    host: *const c_char,
    port: u16,
    _: u16,
    _: *const u8,
    context: *mut c_void,
) {
    // SAFETY: DNS-SD returns the same live context passed to Resolve.
    let context = unsafe { &*context.cast::<ResolveContext>() };
    let service = &context.service;
    if code == POLICY_DENIED {
        push(&context.inbox, Event::Error(Error(code)));
        return;
    }
    // SAFETY: host is a valid C string on success and unused on error.
    let host = (code == 0).then(|| unsafe { dns_name(host) }).flatten();
    let device = host.and_then(|host| resolved_device(&host, port, &service.name));
    push(
        &context.inbox,
        device.map_or_else(
            || Event::ResolveFailed(service.clone()),
            |device| Event::Resolved(service.clone(), device),
        ),
    );
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

type BrowseReply =
    extern "C" fn(Raw, u32, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void);
type ResolveReply = extern "C" fn(
    Raw,
    u32,
    u32,
    i32,
    *const c_char,
    *const c_char,
    u16,
    u16,
    *const u8,
    *mut c_void,
);

#[link(name = "System")]
unsafe extern "C" {
    fn DNSServiceCreateConnection(reference: *mut Raw) -> i32;
    fn DNSServiceBrowse(
        reference: *mut Raw,
        flags: u32,
        interface: u32,
        regtype: *const c_char,
        domain: *const c_char,
        callback: BrowseReply,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceResolve(
        reference: *mut Raw,
        flags: u32,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        callback: ResolveReply,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceRefSockFD(reference: Raw) -> c_int;
    fn DNSServiceProcessResult(reference: Raw) -> i32;
    fn DNSServiceRefDeallocate(reference: Raw);
    fn poll(fds: *mut PollFd, count: u32, timeout: c_int) -> c_int;
}
