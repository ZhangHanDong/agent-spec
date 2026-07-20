# Rust Atlas Runtime Boundary Hints

Rust Atlas can prove paths represented by graph facts and bounded candidate
edges. Some Rust control flow continues through runtime mechanisms that the
current graph cannot prove, such as callback registries or framework routes.
For a disconnected `atlas flow` query, Atlas can expose that stopping point as
a query-time hint without inventing a graph edge.

```bash
agent-spec atlas build --code . --graph .agent-spec/graph
agent-spec atlas flow \
  --from my_crate::dispatch \
  --to my_crate::callback_handler \
  --code . \
  --graph .agent-spec/graph \
  --frozen
```

When fresh source at the query source or its reachable static spine contains a
recognized boundary, the
JSON result includes `runtime_boundaries` and the exact diagnostic
`atlas-flow-runtime-boundary`. Each hint records:

- the source function and exact source site;
- one of `async-task`, `channel`, `callback-registry`, `reflection`, or
  `framework-route`;
- a bounded normalized expression and optional static key;
- unresolved candidate text and up to 16 canonically ordered candidate nodes;
- `authority: query-hint` and `confidence: heuristic`.

Candidate overflow returns no arbitrary prefix and sets
`candidates_truncated`. A query scans at most 8 source-first reachable functions, 200000
source bytes, and 4 hints. Query-level exhaustion sets
`runtime_boundary_truncated` and adds
`atlas-flow-runtime-boundary-truncated`.
Frontier construction and AST scanning share a hash-validated per-file byte
cache. Atlas checks the node and remaining byte budget before admitting another
function or reading another source file, so a large reachable graph cannot do
unbounded source I/O before the visible limit is applied.

Atlas binds each scan to one function AST by graph-node name, signature, and
source span. This prevents a runtime site in a same-line sibling function from
being attributed to the query source; a non-unique selection produces no hint.
Stored and parsed signatures use the same whitespace canonicalization, so a
qualified type such as `crate::Registry` does not suppress a valid scan.
Candidate lookup preserves the displayed source text while canonicalizing
`crate`, `self`, `super`, `Self`, and qualified paths against the source
package, module, or implementation before querying the existing index. A
qualified-self candidate retains its self type, trait path, and member, so an
explicit implementation does not expand to every same-named trait method.
For a bare candidate, Atlas first queries the exact symbol in the source module.
It uses global suffix fallback only when no contextual exact symbol exists, so
an unrelated sibling module's same-named function is not an impossible
continuation.
Generic arguments in those paths are retained for index lookup. Receiver roles
match complete AST identifiers or underscore-delimited suffixes only along the
callable and access chain that produces the receiver. Call arguments, index
values, and literal contents are ignored; names such as `ctx` and `syllabus` do
not become `tx` or `bus` roles.

For reflection hints, Atlas preserves generic candidate text such as
`Message<u8>` in the result while resolving it to the indexed `Message` type
declaration. Reflection lookup keeps only Rust type-namespace declarations, so
a legal same-symbol value function is not an impossible reflection candidate.
Async-task, callback-registry, and framework-route continuations keep only
indexed function declarations. Namespace filtering occurs before the
16-candidate fan-out check.

Callable paths such as `crate::Handler::callback`,
`self::Handler::callback`, and source-relative `Handler::callback` first resolve
the indexed type declaration and then its canonical inherent-implementation
method symbol. This declaration lookup does not weaken the separate rule that
qualified-self candidates retain complete generic identity.

For default trait methods, lowercase `self` and `super` paths resolve from the
module that declares the trait, while `Self` remains scoped to the trait
container. A qualified `<Self as Trait>::member` continuation resolves against
that indexed trait declaration. Atlas derives these distinctions from the
indexed `Contains` relationship instead of guessing from symbol text.

Framework-route hints preserve continuations from both common AST forms:
`route(path, handler)` reads the second argument, while `service(handler)` reads
its single argument. Wrapper calls such as `get(handler)` are unwrapped to the
handler path without turning the wrapper itself into a candidate.

## Authority Boundary

A runtime boundary is an explanation of where the static path stopped. It is
not proof that a candidate executes. Atlas does not persist these hints, add
edges, change the graph fingerprint, or feed them into `impact`, `affected`,
code bindings, lifecycle, or archive evidence.

Source must remain under the canonical code root and its current BLAKE3 hash
must match `Meta.files`. Missing, stale, escaped, non-UTF-8, or unparseable
source produces no hint and cannot expose hints from its persisted descendants.
SCIP and MIR edges expand the scan frontier only when their corresponding
authority layer is fresh. A `found`, `truncated`, `unknown-endpoint`, or
`ambiguous-endpoint` flow also remains inert. This keeps existing authoritative
answers quiet and prevents a heuristic scan from hiding uncertainty.

## Regression Gate

The E3 query corpus contains the offline
`fixtures/atlas/runtime-boundaries` case. Its live probe rebuilds a fresh SCIP
graph with a resolved helper edge, runs the current flow implementation, and
scores the expected continuation path, source evidence, and the exact diagnostic
emitted by that flow. A mechanism may
be promoted to a persisted bounded candidate edge only after its own positive
and negative corpus, inert behavior, deterministic ordering, fan-out policy,
and false-positive measurements pass the roadmap promotion gate.
