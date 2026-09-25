//! A `tracing` layer that counts the instructions each span runs.
//!
//! Every span is keyed by where it was declared, and by the fields that name
//! what it ran on. The card and the monster are two of those. The counts build a tree of
//! those keys, one path per nesting, as the spans nest at run time.
//!
//! The counter is the thread's own hardware counter of user instructions. What
//! the layer spends inside its callbacks is kept off every span. What
//! `tracing` spends reaching the layer cannot be, so [`calibrate`] measures it
//! on empty spans, and a report takes it off.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;

use perf_event::Builder;
use perf_event::events::Hardware;
use tracing::span;
use tracing::subscriber::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// The fields that split a span's key, because they name what it ran on.
const KEYS: [&str; 5] = ["card", "monster", "move_id", "hook", "decision"];

/// A span's key: where it was declared, and its keying fields.
struct Key(String);

/// One path through the tree, and what ran under it.
#[derive(Default)]
struct Node {
    key: String,
    children: HashMap<String, usize>,
    calls: u64,
    inclusive: u64,
}

struct State {
    counter: perf_event::Counter,
    /// Instructions spent inside this layer's callbacks, which no span pays.
    hidden: u64,
    /// The node each open span stands at, and the clock as it was entered.
    open: Vec<(usize, u64)>,
    nodes: Vec<Node>,
}

impl State {
    fn new() -> Self {
        let mut counter = Builder::new(Hardware::INSTRUCTIONS)
            .exclude_kernel(true)
            .exclude_hv(true)
            .build()
            .expect("a hardware instruction counter, which perf_event_paranoid 2 or less allows");
        counter.enable().expect("the counter starts");
        Self {
            counter,
            hidden: 0,
            open: Vec::new(),
            nodes: vec![Node::default()],
        }
    }

    fn raw(&mut self) -> u64 {
        self.counter.read().expect("the counter reads")
    }

    /// The node `key` stands at under the innermost open span.
    fn child(&mut self, key: &str) -> usize {
        let parent = self.open.last().map_or(0, |&(node, _)| node);
        if let Some(&at) = self.nodes[parent].children.get(key) {
            return at;
        }
        let at = self.nodes.len();
        self.nodes.push(Node {
            key: key.to_owned(),
            ..Node::default()
        });
        self.nodes[parent].children.insert(key.to_owned(), at);
        at
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Run `f` on this thread's state, and keep what it spends off every span.
fn counted<R>(f: impl FnOnce(&mut State, u64) -> R) -> R {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.get_or_insert_with(State::new);
        let before = state.raw();
        let clock = before - state.hidden;
        let result = f(state, clock);
        let after = state.raw();
        state.hidden += after - before;
        result
    })
}

/// The layer. Its state is the thread's own, so it holds nothing itself.
pub struct Instructions;

impl<S: Subscriber + for<'a> LookupSpan<'a>> tracing_subscriber::Layer<S> for Instructions {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        counted(|_, _| {
            let metadata = attrs.metadata();
            let mut key = format!("{}::{}", metadata.target(), metadata.name());
            let mut fields = Fields(String::new());
            attrs.record(&mut fields);
            if !fields.0.is_empty() {
                write!(key, "{{{}}}", fields.0).unwrap();
            }
            if let Some(span) = ctx.span(id) {
                span.extensions_mut()
                    .insert(Key(key.replace([' ', ';'], "_")));
            }
        });
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let extensions = span.extensions();
        let Some(Key(key)) = extensions.get::<Key>() else {
            return;
        };
        counted(|state, clock| {
            let node = state.child(key);
            state.open.push((node, clock));
        });
    }

    fn on_exit(&self, _: &span::Id, _: Context<'_, S>) {
        counted(|state, clock| {
            let Some((node, entered)) = state.open.pop() else {
                return;
            };
            state.nodes[node].calls += 1;
            state.nodes[node].inclusive += clock - entered;
        });
    }
}

/// The keying fields a span carries, as `name=value` pairs.
struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if KEYS.contains(&field.name()) {
            if !self.0.is_empty() {
                self.0.push(',');
            }
            write!(self.0, "{}={value:?}", field.name()).unwrap();
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if KEYS.contains(&field.name()) {
            if !self.0.is_empty() {
                self.0.push(',');
            }
            write!(self.0, "{}={value}", field.name()).unwrap();
        }
    }
}

/// What `tracing` spends on a span that the layer cannot keep off the count.
///
/// Some lands on the parent, as the span is built and entered. It grows with
/// how many spans are open above it. The rest lands on the span itself,
/// because the registry leaves it before the layer hears that it has.
#[derive(Debug, Clone, Copy)]
pub struct Overhead {
    /// What a span costs its parent, per call, with only its parent open.
    pub parent: u64,
    /// What each further span open above the parent adds to that.
    pub per_level: u64,
    /// What an empty span counts for itself, per call.
    pub own: u64,
}

impl Overhead {
    /// What a span costs a parent that stands `depth` spans deep, the
    /// outermost span being 1 deep.
    fn parent_at(self, depth: u64) -> u64 {
        self.parent + self.per_level * depth.saturating_sub(1)
    }
}

/// What one empty span costs, measured on this thread under the parent's
/// own spans. It runs before the work being traced, and leaves nothing in
/// the tree.
pub fn calibrate() -> Overhead {
    /// How deep the second measurement nests, to find what a level adds.
    const DEEP: u64 = 16;
    let (parent, own) = empty_spans(0);
    let (deep, _) = empty_spans(DEEP);
    Overhead {
        parent,
        per_level: deep.saturating_sub(parent) / DEEP,
        own,
    }
}

/// What an empty span costs its parent and itself, per call, with `depth`
/// spans open above that parent.
fn empty_spans(depth: u64) -> (u64, u64) {
    const SPANS: u64 = 10_000;
    let above: Vec<_> = (0..depth)
        .map(|_| tracing::trace_span!("above").entered())
        .collect();
    let outer = tracing::trace_span!("calibration");
    {
        let _outer = outer.enter();
        for _ in 0..SPANS {
            let _inner = tracing::trace_span!("empty").entered();
        }
    }
    drop(outer);
    drop(above);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().expect("the calibration ran under the layer");
        let at = state
            .nodes
            .iter()
            .position(|node| node.key == "sts2_trace::layer::calibration")
            .expect("the calibration span was entered");
        let child = state.nodes[at].children["sts2_trace::layer::empty"];
        let (inclusive, children) = (state.nodes[at].inclusive, state.nodes[child].inclusive);
        // Leave nothing of the calibration in the tree being traced.
        state.nodes.truncate(1);
        state.nodes[0].children.clear();
        ((inclusive - children) / SPANS, children / SPANS)
    })
}

/// Every path in the tree as a folded stack, root first, with what its own
/// span ran and how many times it ran. `overhead` comes off for the span and
/// for each child it opened.
///
/// A path through a span whose key names `leave_out` is left out with
/// everything under it.
pub fn folded(overhead: Overhead, leave_out: &str) -> Vec<(String, u64, u64)> {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![(0usize, 0u64, String::new())];
        while let Some((at, depth, path)) = stack.pop() {
            let node = &state.nodes[at];
            let mut own = node.inclusive.saturating_sub(overhead.own * node.calls);
            for &child in node.children.values() {
                let child = &state.nodes[child];
                own = own.saturating_sub(child.inclusive + overhead.parent_at(depth) * child.calls);
            }
            if at != 0 {
                out.push((path.clone(), own, node.calls));
            }
            for &child in node.children.values() {
                let key = &state.nodes[child].key;
                if key.contains(leave_out) {
                    continue;
                }
                let path = match path.is_empty() {
                    true => key.clone(),
                    false => format!("{path};{key}"),
                };
                stack.push((child, depth + 1, path));
            }
        }
        out
    })
}

/// The instructions run under every span whose key names `pattern`, counted
/// once where one sits inside another.
pub fn inclusive_of(pattern: &str) -> u64 {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return 0;
        };
        let mut total = 0;
        let mut stack = vec![0usize];
        while let Some(at) = stack.pop() {
            for &child in state.nodes[at].children.values() {
                match state.nodes[child].key.contains(pattern) {
                    true => total += state.nodes[child].inclusive,
                    false => stack.push(child),
                }
            }
        }
        total
    })
}
