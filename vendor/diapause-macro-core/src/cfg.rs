//! The control-flow graph produced by `lower.rs`: block/terminator data
//! types, the binding table, and post-lowering simplification
//! (goto-chain merging, unreachable-block removal, inlining).

use std::collections::BTreeSet;

pub type BlockId = usize;

/// Identifies one binding (argument, `let`, resume binding, or pattern
/// binding) uniquely across shadowing and scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(pub usize);

#[derive(Debug, Clone)]
pub struct Binding {
    pub ident: syn::Ident,
    pub mutability: Option<syn::Token![mut]>,
    pub kind: BindingKind,
    /// Syntactic type information; resolved recursively by the analysis.
    pub ty: TySource,
    pub borrow: BorrowSource,
    /// Index into the defining block's `stmts` of the `let` statement
    /// that introduced this binding, if it was introduced by one (as
    /// opposed to a function argument, a resume binding, or a
    /// match/`for` pattern).
    pub def_stmt: Option<usize>,
}

/// How a binding was introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    /// Function argument; its type comes from the signature.
    Arg,
    /// Bound by a `let` statement (simple or destructuring).
    Local,
    /// Resume binding of `let r = yield_!(..);`; its type defaults to
    /// the coroutine's resume type.
    Resume,
    /// Bound by a `match`/`while let` arm pattern. There is nowhere to
    /// write a type annotation, so it must not cross a state boundary.
    ArmPat,
    /// Synthetic `__iter{k}` binding holding a `for` loop's iterator.
    ForIter,
    /// Synthetic `__dg{k}` binding holding the coroutine a `yield_all!`
    /// delegates to. Its type follows the operand variable; an
    /// unresolvable type gets a dedicated error message that does not
    /// leak the synthetic name.
    Delegate,
    /// Bound by a destructuring `for` loop pattern. Component types
    /// cannot be derived, so it must not cross a state boundary.
    ForPat,
    /// Bound by a destructuring argument pattern, re-bound by a
    /// synthesized `let` at the top of the entry block. Component types
    /// cannot be derived from the signature, so it must not cross a
    /// state boundary.
    ArgPat,
}

/// Syntactically determined type of a binding.
// One instance per binding; keeping the syn type inline is simpler than
// boxing it (same trade-off as Terminator).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum TySource {
    Unknown,
    /// Explicit annotation, literal suffix, or unambiguous literal kind.
    Known(syn::Type),
    /// `let y = x;`: the type follows the moved binding.
    Moved(BindingId),
    /// The synthetic `__dg{k}` binding of a boxed delegation
    /// (`yield_all!(box sub)`): `Box<T>` where the inner source is the
    /// type of the unboxed coroutine moved into `Box::new`. Spelled as
    /// `::diapause::__private::Box`, so it resolves in `no_std` crates;
    /// the re-export requires diapause's `alloc` feature.
    Boxed(Box<TySource>),
    /// `a..b` / `a..=b`: `Range<T>` / `RangeInclusive<T>` where `T` is
    /// the first endpoint whose type is known.
    Range {
        inclusive: bool,
        start: Box<TySource>,
        end: Box<TySource>,
    },
    /// A `for` loop's iterator: `<T as IntoIterator>::IntoIter` where
    /// the inner source is the type of the iterated expression.
    IntoIter(Box<TySource>),
    /// An inclusive-range `for` loop's iterator: the generated
    /// `__RangeInclusiveIter<T>` wrapper, where the inner source
    /// resolves to `RangeInclusive<T>`. `RangeInclusive` itself is not
    /// stored because its serde impl drops the internal exhaustion
    /// flag, which breaks round-tripping a state suspended after the
    /// final element (see `expand::range_inclusive_iter_def`).
    RangeInclusiveIter(Box<TySource>),
    /// A `for` loop's variable: `<I as Iterator>::Item` where `I` is
    /// the type of the loop's `__iter{k}` binding.
    IterItem(BindingId),
    /// The completion value of a delegation stored without an
    /// annotation (`let r = yield_all!(sub);`):
    /// `<C as Coroutine<R>>::Return` where the inner source is the
    /// delegate coroutine's type and `R` is the outer resume type.
    DelegateReturn(Box<TySource>),
}

/// Classification of a binding's initializer as a borrow.
#[derive(Debug, Clone)]
pub enum BorrowSource {
    NotABorrow,
    /// `let y = &x;` / `let y = &mut x;` with a plain identifier source.
    /// `source` is `None` when the identifier is not a local binding
    /// (e.g. a static); the borrow is still rebuilt by name.
    Direct {
        source_ident: syn::Ident,
        source: Option<BindingId>,
        mutable: bool,
    },
    /// A reference that cannot be reconstructed; the message explains why.
    NonReconstructible {
        why: &'static str,
    },
}

/// A `break`/`continue` written inside an opaque statement but targeting
/// an expanded (yield-containing) loop or labeled block outside it.
/// Lowering rewrites the jump expression into a `__diapause_jump!(k)`
/// marker (`k` indexes `Cfg::opaque_jumps`) and codegen replaces the
/// marker with the real transition; the indirection keeps the statement
/// tokens valid while simplification renumbers blocks.
#[derive(Debug, Clone)]
pub struct OpaqueJump {
    pub kind: OpaqueJumpKind,
    /// Identifiers bound by patterns inside the statement containing the
    /// jump. The transition moves the target's live-in bindings by name
    /// from the jump site, so an inner binding shadowing one of them
    /// would be captured instead; the analysis rejects that combination
    /// (conservatively: any binding in the statement counts).
    pub shadowed: Vec<syn::Ident>,
}

#[derive(Debug, Clone, Copy)]
pub enum OpaqueJumpKind {
    /// Transition to `target`: a loop's exit block for `break`, its
    /// header for `continue`. For a valued `break` out of a
    /// `let`-initializer loop, `store` is the destination binding; the
    /// rewritten tokens bind it with a synthesized `let` just before
    /// the marker.
    Goto {
        target: BlockId,
        store: Option<BindingId>,
    },
    /// A valued `break` out of a loop in the function's tail position:
    /// the marker carries the value and completes the coroutine.
    Complete,
}

#[derive(Debug, Clone)]
pub struct Cfg {
    pub blocks: Vec<Block>,
    pub entry: BlockId,
    /// All bindings, indexed by `BindingId`; the function arguments come
    /// first, in declaration order.
    pub bindings: Vec<Binding>,
    /// All opaque-statement jumps, indexed by the `k` embedded in their
    /// `__diapause_jump!(k)` markers.
    pub opaque_jumps: Vec<OpaqueJump>,
}

impl Cfg {
    /// Successor block ids of `b`: the terminator's successors plus the
    /// targets of opaque jumps in its statements (`Complete` jumps leave
    /// the coroutine and have no successor).
    pub fn successors(&self, b: BlockId) -> impl DoubleEndedIterator<Item = BlockId> + '_ {
        self.blocks[b]
            .terminator
            .successors()
            .chain(
                self.blocks[b]
                    .jumps
                    .iter()
                    .filter_map(|&j| match self.opaque_jumps[j].kind {
                        OpaqueJumpKind::Goto { target, .. } => Some(target),
                        OpaqueJumpKind::Complete => None,
                    }),
            )
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Opaque statements: anything that does not contain `yield_!`.
    pub stmts: Vec<syn::Stmt>,
    pub terminator: Terminator,
    /// Bindings read in this block before any local redefinition,
    /// over-approximated for opaque statements.
    pub uses: BTreeSet<BindingId>,
    /// Bindings introduced in this block.
    pub defs: BTreeSet<BindingId>,
    /// Resume entry point after a yield; always becomes an enum variant.
    pub resume_point: bool,
    /// Indices into `Cfg::opaque_jumps` of the jump markers embedded in
    /// this block's `stmts`.
    pub jumps: Vec<usize>,
    /// Set by simplification: the block has a unique predecessor and is
    /// emitted inline in that predecessor's transition arm instead of
    /// becoming an enum variant.
    pub inline: bool,
}

// A CFG holds a handful of terminators per coroutine; keeping the syn
// expressions inline is simpler than boxing them.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Unconditional transfer (join points, loop back edges).
    Goto(BlockId),
    /// From `if`; an `if` without `else` points `else_` at the join.
    Branch {
        cond: syn::Expr,
        then_: BlockId,
        else_: BlockId,
    },
    /// From `match` and `while let`.
    Match {
        scrutinee: syn::Expr,
        arms: Vec<MatchArm>,
    },
    /// Suspension point.
    Yield {
        value: syn::Expr,
        resume_binding: Option<ResumeBinding>,
        next: BlockId,
    },
    /// From `for` loops: calls `next()` on the stored iterator and
    /// matches `Some(pat) => body / None => exit`.
    IterNext {
        iter: syn::Ident,
        pat: Box<syn::Pat>,
        body: BlockId,
        exit: BlockId,
    },
    /// End of the coroutine body; the value is the trailing expression
    /// or `()`. Early `return` is handled by expression rewriting, not
    /// by this terminator.
    Return(syn::Expr),
    /// Statically unreachable fall-through (the diverging `else` block
    /// of a lowered `let ... else` left the block open). Expands to a
    /// bare diverging expression rather than a `Return`-style
    /// `let __ret: T = ..;`, which would put the divergence in value
    /// position and trip `clippy::diverging_sub_expression` on the
    /// user's spans.
    Unreachable(syn::Expr),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: syn::Pat,
    pub guard: Option<syn::Expr>,
    pub body: BlockId,
}

#[derive(Debug, Clone)]
pub struct ResumeBinding {
    pub binding: BindingId,
    pub mutability: Option<syn::Token![mut]>,
    pub ty: Option<syn::Type>,
}

impl Terminator {
    /// Successor block ids, in the order a `match`/`if` would evaluate
    /// them. Allocation-free: fixed-arity terminators pack their targets
    /// into a small array, and `Match` walks its arms slice directly.
    pub fn successors(&self) -> Successors<'_> {
        match self {
            Terminator::Goto(b) => Successors::Fixed([Some(*b), None].into_iter()),
            Terminator::Branch { then_, else_, .. } => {
                Successors::Fixed([Some(*then_), Some(*else_)].into_iter())
            }
            Terminator::Match { arms, .. } => Successors::Match(arms.iter()),
            Terminator::Yield { next, .. } => Successors::Fixed([Some(*next), None].into_iter()),
            Terminator::IterNext { body, exit, .. } => {
                Successors::Fixed([Some(*body), Some(*exit)].into_iter())
            }
            Terminator::Return(_) | Terminator::Unreachable(_) => {
                Successors::Fixed([None, None].into_iter())
            }
        }
    }
}

/// Iterator over a terminator's successor block ids. `Fixed` covers
/// terminators with at most two successors (the `None` slots are
/// skipped); `Match` has one successor per arm.
pub enum Successors<'a> {
    Fixed(std::array::IntoIter<Option<BlockId>, 2>),
    Match(std::slice::Iter<'a, MatchArm>),
}

impl Iterator for Successors<'_> {
    type Item = BlockId;

    fn next(&mut self) -> Option<BlockId> {
        match self {
            Successors::Fixed(it) => it.find_map(|s| s),
            Successors::Match(it) => it.next().map(|a| a.body),
        }
    }
}

impl DoubleEndedIterator for Successors<'_> {
    fn next_back(&mut self) -> Option<BlockId> {
        match self {
            Successors::Fixed(it) => it.rfind(|s| s.is_some()).flatten(),
            Successors::Match(it) => it.next_back().map(|a| a.body),
        }
    }
}

/// Merges single-predecessor `Goto` chains, drops unreachable blocks,
/// and marks the remaining single-predecessor blocks (branch/match arm
/// targets) as inline. Resume points always stay separate blocks: they
/// are the state-machine's resume entry variants. So do opaque-jump
/// targets: they are entered by `continue '__dispatch`, which only
/// reaches dispatchable variants.
pub(crate) fn simplify(cfg: &mut Cfg) {
    remove_unreachable(cfg);
    merge_goto_chains(cfg);
    remove_unreachable(cfg);
    mark_inline(cfg);
}

/// Copies the joins on the path a run takes when it does not suspend,
/// so that path stays plain Rust (see [`duplicate_joins`]), then marks
/// the copies inline. Expansion runs it after [`simplify`]; the
/// lowering's own tests see the CFG without it.
pub(crate) fn inline_joins(cfg: &mut Cfg) {
    duplicate_joins(cfg);
    mark_inline(cfg);
}

/// Marks each block with one predecessor inline, but for the entry,
/// resume points, and opaque-jump targets.
fn mark_inline(cfg: &mut Cfg) {
    let preds = pred_edge_counts(cfg);
    let jump_target = jump_targets(cfg);
    let entry = cfg.entry;
    for (i, b) in cfg.blocks.iter_mut().enumerate() {
        b.inline = i != entry && !b.resume_point && preds[i] == 1 && !jump_target[i];
    }
}

/// Clones each join block, together with the tree of blocks it alone
/// leads to, once for each predecessor edge past the first. Every copy
/// then has one predecessor and is inlined into it.
///
/// A join that stays a block of its own becomes a state variant, and
/// every run that reaches it, suspending or not, moves the live locals
/// into the state enum and back out through the dispatch loop. A join
/// after an `if` or a `match` whose arms can yield is on the common path
/// of a body that rarely suspends. Duplicating the join keeps that path
/// in plain Rust: only a yield writes the state.
///
/// The join's tree is every block reached from it through blocks with
/// one predecessor, up to blocks that already have several. It is copied
/// whole, because a block in it with one more predecessor would become a
/// variant, and a `match` arm's bindings would leave its scope. A yield in
/// the tree is copied with its resume point, so each copy is a resume
/// point of its own whose one predecessor is its own yield: more states,
/// and no cost at run time.
///
/// Each opaque jump has one owning block, so a copied block's
/// `__diapause_jump!` markers are renumbered to fresh copies of their
/// jumps. A jump's target is never in the tree, which stops at jump
/// targets, so the copies jump where the originals do.
///
/// A join qualifies only if neither it nor its tree defines a direct
/// borrow or lies on a cycle. A direct borrow needs a unique defining
/// block, since its `let` may be removed and rebuilt (see
/// `analyze_cfg::removed_statements`). A cycle is a loop, whose header
/// stays a variant.
///
/// Only the path that does not suspend needs the copy. A path that
/// resumed from a yield came through the dispatch loop already, so it can
/// reach the join as a variant. A block is hot where the entry reaches it
/// without passing a resume point. Where some edges into a join are hot
/// and some are not, each hot edge gets a copy and the others keep the
/// join. Where every edge is hot, the first keeps it and each later one
/// gets a copy.
///
/// The pass repeats to a fixed point, since a copied tree adds
/// predecessors to the joins below it. A budget of copied statements
/// bounds the growth. The hot edges come first: since the resumed paths
/// take no copies then, a chain of joins with a yield before each grows by
/// one copy of each tree. What the budget has left then goes to every
/// edge, as fewer variants make a smaller state enum to compile. There a
/// chain of joins doubles at each step, and past the budget a join stays
/// a variant.
fn duplicate_joins(cfg: &mut Cfg) {
    let total: usize = cfg.blocks.iter().map(|b| b.stmts.len() + 1).sum();
    let mut budget = (total * JOIN_BUDGET_FACTOR).max(JOIN_BUDGET_FLOOR);
    // The hot edges first, then every edge with what the budget has left.
    let mut hot_only = true;
    loop {
        let preds = pred_edge_counts(cfg);
        let jump_target = jump_targets(cfg);
        let cyclic = on_cycles(cfg);
        let hot = hot_blocks(cfg);
        let hot_preds = hot_pred_edge_counts(cfg, &hot);
        let candidate = (0..cfg.blocks.len()).find_map(|b| {
            if b == cfg.entry || preds[b] < 2 || cfg.blocks[b].resume_point || jump_target[b] {
                return None;
            }
            if hot_only && hot_preds[b] == 0 {
                return None;
            }
            let tree = join_tree(cfg, &preds, &jump_target, &cyclic, b)?;
            let size: usize = tree.iter().map(|&t| cfg.blocks[t].stmts.len() + 1).sum();
            let all_hot = !hot_only || hot_preds[b] == preds[b];
            let copies = if all_hot { preds[b] - 1 } else { hot_preds[b] };
            let cost = size * copies;
            (cost <= budget).then_some((b, tree, cost, all_hot))
        });
        let Some((b, tree, cost, all_hot)) = candidate else {
            if std::mem::replace(&mut hot_only, false) {
                continue;
            }
            break;
        };
        budget -= cost;
        // Where every edge into `b` is hot, keep the first and point each
        // later one at a copy of the whole tree. Otherwise each hot edge
        // gets a copy, and the resumed ones keep `b`.
        let mut seen = false;
        for p in 0..cfg.blocks.len() {
            let mut term = std::mem::replace(
                &mut cfg.blocks[p].terminator,
                Terminator::Return(syn::parse_quote!(())),
            );
            for slot in successor_slots(&mut term) {
                if *slot != b {
                    continue;
                }
                let copied = match all_hot {
                    true => std::mem::replace(&mut seen, true),
                    false => hot[p],
                };
                if copied {
                    *slot = copy_tree(cfg, &tree);
                }
            }
            cfg.blocks[p].terminator = term;
        }
    }
}

/// `b` and every block reached from it through blocks with one
/// predecessor, or nothing where one of them may not be copied.
fn join_tree(
    cfg: &Cfg,
    preds: &[usize],
    jump_target: &[bool],
    cyclic: &[bool],
    b: BlockId,
) -> Option<Vec<BlockId>> {
    let mut tree = Vec::new();
    let mut stack = vec![b];
    while let Some(t) = stack.pop() {
        let blk = &cfg.blocks[t];
        let copyable = !cyclic[t]
            && !blk.defs.iter().any(|d| {
                matches!(cfg.bindings[d.0].borrow, BorrowSource::Direct { .. })
            });
        if !copyable {
            return None;
        }
        tree.push(t);
        stack.extend(
            cfg.blocks[t]
                .terminator
                .successors()
                .filter(|&s| preds[s] == 1 && !jump_target[s]),
        );
    }
    Some(tree)
}

/// Appends a copy of `tree`, whose first block is its root, with edges
/// inside it pointing at the copies. Returns the root's copy.
fn copy_tree(cfg: &mut Cfg, tree: &[BlockId]) -> BlockId {
    let base = cfg.blocks.len();
    let index: std::collections::BTreeMap<BlockId, BlockId> = tree
        .iter()
        .enumerate()
        .map(|(i, &t)| (t, base + i))
        .collect();
    for &t in tree {
        let mut copy = cfg.blocks[t].clone();
        copy.inline = false;
        for slot in successor_slots(&mut copy.terminator) {
            if let Some(&to) = index.get(slot) {
                *slot = to;
            }
        }
        if !copy.jumps.is_empty() {
            let mut renumbered = std::collections::BTreeMap::new();
            for k in &mut copy.jumps {
                let fresh = cfg.opaque_jumps.len();
                cfg.opaque_jumps.push(cfg.opaque_jumps[*k].clone());
                renumbered.insert(*k, fresh);
                *k = fresh;
            }
            let mut renumber = Renumber(&renumbered);
            for stmt in &mut copy.stmts {
                syn::visit_mut::VisitMut::visit_stmt_mut(&mut renumber, stmt);
            }
        }
        cfg.blocks.push(copy);
    }
    base
}

/// Renumbers the `__diapause_jump!` markers of a copied block in place, so
/// each stays the statement or expression it was.
struct Renumber<'a>(&'a std::collections::BTreeMap<usize, usize>);

impl syn::visit_mut::VisitMut for Renumber<'_> {
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let tokens = std::mem::take(&mut mac.tokens);
        mac.tokens = match mac.path.is_ident("__diapause_jump") {
            true => renumber_marker_body(tokens, self.0),
            false => renumber_markers(tokens, self.0),
        };
    }
}

/// A marker's own tokens, `k [, value]`, with `k` renumbered and any
/// marker in the value too.
fn renumber_marker_body(
    tokens: proc_macro2::TokenStream,
    renumbered: &std::collections::BTreeMap<usize, usize>,
) -> proc_macro2::TokenStream {
    use proc_macro2::{Literal, TokenTree};

    let mut inner: Vec<TokenTree> = tokens.into_iter().collect();
    if let Some(TokenTree::Literal(lit)) = inner.first()
        && let Ok(k) = lit.to_string().parse::<usize>()
        && let Some(&to) = renumbered.get(&k)
    {
        let mut fresh = Literal::usize_unsuffixed(to);
        fresh.set_span(lit.span());
        inner[0] = TokenTree::Literal(fresh);
    }
    renumber_markers(inner.into_iter().collect(), renumbered)
}

/// `tokens` with each `__diapause_jump!(k ..)` marker's `k` replaced by
/// its entry in `renumbered`, markers nested in another's value included.
fn renumber_markers(
    tokens: proc_macro2::TokenStream,
    renumbered: &std::collections::BTreeMap<usize, usize>,
) -> proc_macro2::TokenStream {
    use proc_macro2::{Group, TokenTree};

    let mut out = Vec::new();
    let mut marker = 0u8;
    for tt in tokens {
        let tt = match tt {
            TokenTree::Ident(ref ident) if ident == "__diapause_jump" => {
                marker = 1;
                tt
            }
            TokenTree::Punct(ref p) if marker == 1 && p.as_char() == '!' => {
                marker = 2;
                tt
            }
            TokenTree::Group(group) => {
                let stream = match std::mem::take(&mut marker) == 2 {
                    true => renumber_marker_body(group.stream(), renumbered),
                    false => renumber_markers(group.stream(), renumbered),
                };
                let mut fresh = Group::new(group.delimiter(), stream);
                fresh.set_span(group.span());
                TokenTree::Group(fresh)
            }
            tt => {
                marker = 0;
                tt
            }
        };
        out.push(tt);
    }
    out.into_iter().collect()
}

/// How many times its own size a body may grow by copied joins.
const JOIN_BUDGET_FACTOR: usize = 1;

/// The least number of statements [`duplicate_joins`] may copy, for a
/// small body whose own size would leave too little room.
const JOIN_BUDGET_FLOOR: usize = 128;

/// The terminator's successor slots, for retargeting one edge at a time.
fn successor_slots(t: &mut Terminator) -> Vec<&mut BlockId> {
    match t {
        Terminator::Goto(b) => vec![b],
        Terminator::Branch { then_, else_, .. } => vec![then_, else_],
        Terminator::Match { arms, .. } => arms.iter_mut().map(|a| &mut a.body).collect(),
        Terminator::Yield { next, .. } => vec![next],
        Terminator::IterNext { body, exit, .. } => vec![body, exit],
        Terminator::Return(_) | Terminator::Unreachable(_) => Vec::new(),
    }
}

/// Whether each block lies on a cycle (reaches itself), over terminator
/// and opaque-jump edges.
fn on_cycles(cfg: &Cfg) -> Vec<bool> {
    let n = cfg.blocks.len();
    (0..n)
        .map(|start| {
            let mut seen = vec![false; n];
            let mut stack: Vec<BlockId> = cfg.successors(start).collect();
            while let Some(b) = stack.pop() {
                if b == start {
                    return true;
                }
                if !std::mem::replace(&mut seen[b], true) {
                    stack.extend(cfg.successors(b));
                }
            }
            false
        })
        .collect()
}

/// Predecessor edge counts, opaque-jump edges included.
/// The blocks the entry reaches without passing a resume point: the path
/// that does not suspend.
fn hot_blocks(cfg: &Cfg) -> Vec<bool> {
    let mut hot = vec![false; cfg.blocks.len()];
    let mut stack = vec![cfg.entry];
    while let Some(b) = stack.pop() {
        if std::mem::replace(&mut hot[b], true) {
            continue;
        }
        for s in cfg.successors(b) {
            if !hot[s] && !cfg.blocks[s].resume_point {
                stack.push(s);
            }
        }
    }
    hot
}

/// How many edges into each block come from `hot` blocks.
fn hot_pred_edge_counts(cfg: &Cfg, hot: &[bool]) -> Vec<usize> {
    let mut counts = vec![0; cfg.blocks.len()];
    for (b, &is_hot) in hot.iter().enumerate() {
        if is_hot {
            for s in cfg.successors(b) {
                counts[s] += 1;
            }
        }
    }
    counts
}

fn pred_edge_counts(cfg: &Cfg) -> Vec<usize> {
    let mut counts = vec![0usize; cfg.blocks.len()];
    for b in 0..cfg.blocks.len() {
        for s in cfg.successors(b) {
            counts[s] += 1;
        }
    }
    counts
}

/// Blocks entered by an opaque jump.
fn jump_targets(cfg: &Cfg) -> Vec<bool> {
    let mut out = vec![false; cfg.blocks.len()];
    for b in &cfg.blocks {
        for &j in &b.jumps {
            if let OpaqueJumpKind::Goto { target, .. } = cfg.opaque_jumps[j].kind {
                out[target] = true;
            }
        }
    }
    out
}

fn merge_goto_chains(cfg: &mut Cfg) {
    loop {
        // A jump target's opaque-jump edges are part of its pred count,
        // so it never qualifies for merging (any `Goto` into it makes
        // preds >= 2, and without one there is nothing to merge from).
        let preds = pred_edge_counts(cfg);
        let mut pair = None;
        for (b, blk) in cfg.blocks.iter().enumerate() {
            if let Terminator::Goto(c) = &blk.terminator {
                let c = *c;
                if c != b && c != cfg.entry && preds[c] == 1 && !cfg.blocks[c].resume_point {
                    pair = Some((b, c));
                    break;
                }
            }
        }
        let Some((b, c)) = pair else { break };
        // Absorb c into b; c becomes an unreachable tombstone that the
        // caller removes afterwards.
        let mut stmts = std::mem::take(&mut cfg.blocks[c].stmts);
        let term = std::mem::replace(
            &mut cfg.blocks[c].terminator,
            Terminator::Return(syn::parse_quote!(())),
        );
        let uses = std::mem::take(&mut cfg.blocks[c].uses);
        let defs = std::mem::take(&mut cfg.blocks[c].defs);
        let jumps = std::mem::take(&mut cfg.blocks[c].jumps);
        // c's statements land after b's existing ones, so the def_stmt
        // indices of c's bindings shift by b's statement count.
        let offset = cfg.blocks[b].stmts.len();
        for d in &defs {
            if let Some(i) = &mut cfg.bindings[d.0].def_stmt {
                *i += offset;
            }
        }
        let bb = &mut cfg.blocks[b];
        bb.stmts.append(&mut stmts);
        bb.terminator = term;
        for u in uses {
            if !bb.defs.contains(&u) {
                bb.uses.insert(u);
            }
        }
        bb.defs.extend(defs);
        // Jump indices are global (into `cfg.opaque_jumps`), so moving
        // the statements does not invalidate the embedded markers.
        bb.jumps.extend(jumps);
    }
}

fn remove_unreachable(cfg: &mut Cfg) {
    let mut reachable = vec![false; cfg.blocks.len()];
    let mut stack = vec![cfg.entry];
    while let Some(b) = stack.pop() {
        if std::mem::replace(&mut reachable[b], true) {
            continue;
        }
        stack.extend(cfg.successors(b));
    }
    if reachable.iter().all(|r| *r) {
        return;
    }
    let mut remap = vec![usize::MAX; cfg.blocks.len()];
    let mut next = 0;
    for (i, r) in reachable.iter().enumerate() {
        if *r {
            remap[i] = next;
            next += 1;
        }
    }
    let blocks = std::mem::take(&mut cfg.blocks);
    cfg.blocks = blocks
        .into_iter()
        .enumerate()
        .filter(|(i, _)| reachable[*i])
        .map(|(_, b)| b)
        .collect();
    for b in &mut cfg.blocks {
        retarget(&mut b.terminator, &remap);
    }
    // Retarget the jumps of surviving blocks. A jump keeps its target
    // reachable (`successors` above traverses jump edges), so no
    // surviving jump can point at a removed block; entries owned by
    // removed blocks become unreferenced and stay stale.
    let live_jumps: Vec<usize> = cfg
        .blocks
        .iter()
        .flat_map(|b| b.jumps.iter().copied())
        .collect();
    for j in live_jumps {
        if let OpaqueJumpKind::Goto { target, .. } = &mut cfg.opaque_jumps[j].kind {
            *target = remap[*target];
        }
    }
    cfg.entry = remap[cfg.entry];
}

fn retarget(t: &mut Terminator, remap: &[usize]) {
    match t {
        Terminator::Goto(b) => *b = remap[*b],
        Terminator::Branch { then_, else_, .. } => {
            *then_ = remap[*then_];
            *else_ = remap[*else_];
        }
        Terminator::Match { arms, .. } => {
            for arm in arms {
                arm.body = remap[arm.body];
            }
        }
        Terminator::Yield { next, .. } => *next = remap[*next],
        Terminator::IterNext { body, exit, .. } => {
            *body = remap[*body];
            *exit = remap[*exit];
        }
        Terminator::Return(_) | Terminator::Unreachable(_) => {}
    }
}
