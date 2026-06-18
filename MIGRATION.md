# Migrating to bpaf 0.10

0.10 is a deliberately breaking release with one headline change: the per-combinator "parser zoo" (`ParseFlag`, `ParseArgument`, `ParseMany`, …) is replaced by a single type-state builder, `Cx<I>`. **Fluent chains, every method name and arity, and the entire derive API are unchanged.** If you never wrote a `Parse*` type by name and never stored a parser in a `let`/field/return with an explicit type, your code very likely compiles untouched. What changes is how you *name* a parser when you have to.

## The mental model

`Cx<I>` wraps an inner state marker `I`, and one blanket impl makes `Cx<I>` a `Parser` exactly when `I` is a finished parser. Every constructor and every combinator returns some `Cx<…>`, so a whole chain stays inside one family of types. A half-built `Cx<Named>` is *not* a parser yet — you finish it with a consumer (`.switch()`, `.argument::<T>(..)`, …). When the compiler needs a name, you have exactly two options, and that is the whole migration:

- name the concrete `Cx<…>` type, or
- erase it with `.boxed()` to `Box<dyn Parser<T>>` (unchanged from 0.9).

## What you need to change

### 1. Explicitly-named parser types

If you wrote a `Parse*` type in a `let`, struct field, or function return, replace it with the `Cx<…>` form (or `.boxed()`):

```rust
// before
let verbose: ParseFlag<bool> = short('v').switch();
fn size() -> ParseArgument<usize> { long("size").argument("SIZE") }

// after — name the Cx type…
let verbose: Cx<Flag<bool>> = short('v').switch();
fn size() -> Cx<Argument<usize>> { long("size").argument("SIZE") }

// …or erase it (usually simpler for fields / heterogeneous storage)
fn size() -> Box<dyn Parser<usize>> { long("size").argument("SIZE").boxed() }
```

The rename drops the `Parse` prefix and the name moves *inside* `Cx<…>` (e.g. `ParseArgument<u32>` → `Cx<Argument<u32>>`); nesting follows the chain (`ParseMany<ParseFlag<bool>>` → `Cx<Many<Cx<Flag<bool>>, Vec<bool>, bool>>`). The markers live in `bpaf::parsers`, but you rarely name them directly — `Cx<…>` is the handle.

| 0.9 type | 0.10 inner marker (used as `Cx<marker>`) |
|---|---|
| `NamedArg` | `Named` |
| `ParseFlag<T>` | `Flag<T>` |
| `ParseArgument<T>` | `Argument<T>` |
| `ParsePositional<T>` | `Positional<T>` |
| `ParseAny<T>` | `Anything<T>` |
| `ParseCommand<T>` | `Command<T>` |
| `ParseMany<P>` / `ParseSome<P>` / `ParseCollect<P,C,T>` | `Many<P, C, T>` (merged — see §3) |
| `ParseOptional<P>` | `Optional<P>` |
| `ParseCount<P,T>` | `Count<P, T>` |
| `ParseMap<…>` | `Map<…>` |
| `ParseWith<…>` | `Parsed<…>` (note: not `With`) |
| `ParseGuard<P,F>` | `Guard<P, F>` |
| `ParseFallback<P,T>` / `ParseFallbackWith<…>` | `Fallback<P, T>` / `FallbackWith<…>` |
| `ParseCon<P>` | `Con<P>` (from `construct!`) |
| `ParseOrElse<T>` | `Alt<T>` (from `construct!([..])`, note: not `OrElse`) |
| `ParsePure<T>` / `ParsePureWith<…>` | `Pure<T>` / `PureWith<…>` |
| `ParseFail<T>` | `Fail<T>` |

The rest follow the same drop-the-`Parse` pattern (`Hide`, `Last`, `Usage`, `GroupHelp`, `WithGroupHelp`, `Adjacent`); the full set is re-exported from `bpaf::parsers`.

### 2. `NamedArg` (temporary alias)

`NamedArg` still exists as a `#[deprecated]` alias for `Cx<Named>`, so existing references compile with a warning for this one release. Update them:

```rust
// before                       // after
fn name() -> NamedArg { ... }   fn name() -> Cx<Named> { ... }
```

`OptionParser::help_parser`, `OptionParser::version_parser`, and `batteries::toggle_flag` now take `Cx<Named>` (the alias still coerces, but update before it's removed in a later release).

### 3. `many` / `some` / `collect` merged into `Many`

`ParseMany`, `ParseSome`, and `ParseCollect` are now one bounded `Many<P, C, T>`. The methods are unchanged — only the named type differs (`Cx<Many<P, C, T>>`). Two consequences:

- New bounded-repetition methods are available for free: `take(n)`, `at_least(n, msg)`, `in_range(range, msg)`.
- **Behavioral change:** a `collect` parser now renders as *optional* (`[ARG]...`) in `--help` rather than required, matching that it accepts zero items — the way `many` always has. If you relied on `collect` showing as required, add a `req_flag`/guard or use `at_least(1, ..)`.

### 4. `req_flag` return type

`req_flag` now returns the concrete `Cx<Flag<T>>` instead of an opaque `impl Parser<T>` — strictly more capable (you can name it). The only break is if you annotated its result as `impl Parser<T>`; drop that and let it be `Cx<Flag<T>>`, or call `.boxed()`.

## What does NOT change

- Every fluent chain, method name, and arity — `short('v').long("verbose").help("…").switch()` is identical.
- The whole derive API (`#[derive(Bpaf)]`, every `#[bpaf(...)]` attribute).
- `construct!`, `choice` / `choice_with`, `.to_options()`, `.run()` / `.run_inner()`, `.boxed()`.
- `OptionParser<T>` and everything after `.to_options()`.

## New in 0.10 (optional)

- `start_adjacent` (combinatoric `Cx<Con<_>>::start_adjacent`, derive `#[bpaf(start_adjacent)]`) — left-anchored adjacency: the consumed block must begin at the current position with nothing unparsed to its left.
- `Parser::take` / `at_least` / `in_range` — bounded repetition built on the unified `Many`.
- `cx()` — lift any hand-written `Parser` implementation into the `Cx` family.
- `Parser` carries a `#[diagnostic::on_unimplemented]` that tells you which consumer to add when you use a half-built builder where a finished parser is required.
