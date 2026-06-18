In `--help` message `req_flag` look similarly to [`switch`](crate::Cx::switch) and
[`flag`](crate::Cx::flag)

> --help

Since parser uses `req_flag` it succeeds exactly 0 times if there's no parameters

>

If it was specified - `count` tracks it a discards parsed values

> -vvv
> --verbose --verbose
