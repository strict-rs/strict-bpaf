#### Switch parser

Let's start with the simplest possible one - a simple switch that gets parsed into a `bool`.

First of all - the switch needs a name - you can start with [`short`] or [`long`] and add more
names if you want: `long("simple")` or `short('s').long("simple")`. This gives something with
the type [`Cx<Named>`](crate::Cx):

```rust
# use bpaf::*;
use bpaf::{Cx, parsers::Named};
fn simple_switch() -> Cx<Named> {
    short('s').long("simple")
}
```

From `Cx<Named>` you make a switch parser by calling [`switch`](crate::Cx::switch). Usually, you
do it right away without assigning `Cx<Named>` to a variable.

```rust
# use bpaf::*;
fn simple_switch() -> impl Parser<bool> {
    short('s').long("simple").switch()
}
```

The switch parser we just made implements trait [`Parser`] and to run it you convert it to [`OptionParser`] with
[`Parser::to_options`] and run it with [`OptionParser::run`]

Full example with some sample inputs and outputs:
#![cfg_attr(not(doctest), doc = include_str!("docs2/compose_basic_switch.md"))]


With [`help`](crate::Cx) you can attach a help message that will be used in `--help` output.
