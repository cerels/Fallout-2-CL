# Prefer boring Rust over idiomatic cleverness

Status: accepted

This project is being rewritten in Rust (ADR 0004) specifically to be maintainable long-term, not
to be a showcase of the language. Rust lets you build elaborate ownership graphs, lifetime-heavy
APIs, and generic/trait-object machinery — and all of that is exactly what makes a codebase
"terrifyingly difficult for a human to maintain" once nobody holds the whole design in their head
at once, AI included.

Decision: default to the simplest ownership pattern that works. Clone data rather than threading
lifetimes through everything; use `Rc<RefCell<>>` for shared mutable state instead of fighting
the borrow checker; avoid `unsafe`; avoid heavy generics, trait-object machinery, and macro magic
unless there's no simpler way. Optimize for "a competent programmer who doesn't know Rust well can
still trace what a function does," not for cleverness or micro-performance. Keep modules small
and flat rather than deep trait hierarchies, with a short doc-comment at the top of each module
stating what it owns and why.

This is a deliberate trade-off (maintainability over idiomatic Rust / micro-performance), and it
happens to reinforce the egui choice in ADR 0004: immediate-mode GUIs already avoid the kind of
retained-widget-tree/shared-state graphs that frameworks like iced tend to need.
