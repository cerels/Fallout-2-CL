# Show indeterminate progress, not a byte-level download bar

The user asked for some loading/progress indicator when the launcher downloads a campaign, mod,
or RPU; today there is none anywhere in the codebase (old Python backend/gui had none either),
and `rust/src/gui/jobs.rs` spawns a plain `std::thread` per job that reports exactly one terminal
`JobOutcome` over an `mpsc` channel polled once per frame, with `rust/src/backend/http.rs` doing a
single blocking `read_to_end` per request and no `Content-Length`-based byte tracking available.
We decided to show an indeterminate spinner plus discrete stage text (e.g. "Downloading..." ->
"Installing..." -> "Done"/"Failed") inline on the row/button that triggered the job, rather than a
true byte-level percentage bar; quick operations with no real download/extract step (an
update-check, a delete) just show a plain "Working..." spinner with no stage breakdown. This needs
only a small intermediate-message variant added to the existing `JobOutcome`-style channel, polled
the same way as today.

## Consequences

Real byte-level progress was considered and explicitly rejected: it would require switching the
HTTP layer to chunked/streaming reads with `Content-Length` parsing and introducing a generic
progress-callback/event mechanism, both of which cut against ADR 0005's commitment to the simplest
thing that works. The indeterminate+stage-text approach already answers what the user actually
asked for ("let me know something is happening"), not a request for an exact percentage, so the
job-running model in `jobs.rs` and the blocking request-per-call shape in `http.rs` stay as they are.
