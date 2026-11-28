Code Quality & Engineering Standards

These policies are mandatory. Treat code quality as a first-class engineering requirement, not an optional refinement. Your responsibility is not merely to make code work, but to leave the codebase cleaner, simpler, more consistent, and more maintainable than you found it.

1. General Engineering Principles
Prefer simple, readable, maintainable solutions over clever or unnecessarily abstract ones.
Write code for the next engineer who must understand, debug, modify, and maintain it.
Every abstraction must justify its existence. Do not introduce abstractions speculatively.
Avoid unnecessary indirection, boilerplate, duplication, and complexity.
Prefer established patterns already present in the codebase over introducing new conventions.
Do not introduce code without understanding its surrounding architecture and responsibilities.
Avoid speculative generalization, premature optimization, and unnecessary extensibility.
Favor explicit behavior over implicit behavior.
Every line of code should serve a discernible purpose.
Do not optimize for minimizing lines of code. Optimize for minimizing cognitive complexity.
2. No Slop

The following are considered code quality defects:

Dead code, unused functions, unused fields, and unnecessary imports.
Unnecessary clones, allocations, conversions, and intermediate collections.
Redundant conditionals, nested branches, and duplicated logic.
Overly generic functions and abstractions that have only one practical implementation.
Excessive trait usage, especially traits introduced solely to wrap a single implementation.
Unnecessary wrapper types and newtypes without meaningful semantic or safety benefits.
Deeply nested modules that add organizational overhead without improving boundaries.
Functions that perform unrelated responsibilities.
Excessively long functions that obscure control flow.
Comments that simply restate what the code already expresses.
TODOs, FIXMEs, and placeholder implementations introduced without justification.
Silent error suppression and ignored Result values.
Arbitrary constants scattered throughout the codebase.
Unnecessary configuration options and feature flags.
Defensive programming against impossible states when the type system can express the invariant.
Compatibility layers for APIs or behaviors that no longer need to be supported.

Do not introduce any of these patterns merely because they are convenient.

3. Rust-Specific Standards
Ownership and Memory Management
Prefer borrowing over cloning when ownership does not need to be transferred.
Avoid .clone() as a convenience mechanism. Every clone should have a reason.
Prefer &str over String and &[T] over Vec<T> for borrowed inputs where appropriate.
Avoid unnecessary heap allocations.
Do not use Arc, Rc, Mutex, or RwLock without a concrete ownership or concurrency requirement.
Prefer stack allocation when appropriate and practical.
Do not introduce interior mutability without a clear reason.
Avoid unnecessary lifetime annotations when lifetime elision is sufficient.
Error Handling
Use Result for recoverable failures and propagate errors appropriately.
Never silently discard errors.
Do not use .unwrap() or .expect() in production paths unless an invariant makes failure genuinely impossible and that invariant is clear.
Use .expect() with a meaningful explanation when asserting an internal invariant.
Avoid converting errors into strings prematurely.
Preserve error context and causal information.
Define domain-specific error types when they meaningfully improve API clarity or error handling.
Avoid creating elaborate error hierarchies for trivial functionality.
Do not use panics as substitutes for ordinary error handling.
Types and APIs
Use the type system to encode meaningful invariants.
Prefer enums over stringly typed state.
Prefer exhaustive matching over catch-all branches when appropriate.
Avoid public APIs unless external visibility is actually required.
Default to the narrowest visibility possible.
Avoid unnecessary generic parameters and trait bounds.
Do not introduce traits merely to make code appear more abstract or extensible.
Prefer concrete types until a genuine need for polymorphism emerges.
Avoid unnecessary conversions between equivalent representations.
Idiomatic Rust
Follow established Rust conventions and standard library idioms.
Prefer iterators where they improve clarity, not simply because they are shorter.
Avoid complicated iterator chains that obscure behavior.
Use pattern matching appropriately.
Avoid unnecessary Box, Pin, and dynamic dispatch.
Use Cow only when borrowed-or-owned semantics provide a real benefit.
Use const and static appropriately.
Avoid unsafe Rust unless absolutely necessary.
Every unsafe block must have a clear safety justification documenting its invariants.
Never use unsafe code merely to avoid a small allocation or inconvenience.
4. Architecture and Workspace Organization
Respect existing crate boundaries and module responsibilities.
Keep dependencies flowing in a deliberate direction.
Avoid circular architectural dependencies.
Do not introduce cross-crate dependencies when a simpler module-level solution is sufficient.
Keep implementation details private.
Avoid turning shared crates into dumping grounds for miscellaneous utilities.
Do not create a new crate for functionality that belongs naturally within an existing crate.
Conversely, do not overload an existing crate with unrelated responsibilities.
Avoid unnecessary re-export layers and facade modules.
Keep domain logic separate from infrastructure concerns where the architecture warrants it.
Do not duplicate domain rules across multiple crates.
Prefer a single authoritative implementation of shared behavior.
Avoid introducing architectural patterns, such as repositories, service layers, factories, or dependency injection frameworks, without a concrete requirement.

Before introducing a new crate, module, or architectural abstraction, determine whether the existing structure already provides an appropriate home.

5. Dependencies
Prefer the Rust standard library when it provides a sufficiently good solution.
Before adding a dependency, investigate whether existing dependencies or internal utilities already solve the problem.
Avoid dependencies that introduce disproportionate complexity for trivial functionality.
Do not add multiple crates that solve substantially the same problem.
Avoid unnecessary transitive dependency growth.
Do not introduce a dependency solely to avoid writing a few straightforward lines of code.
Prefer mature, maintained, well-established dependencies.
Keep dependency versions and feature selections deliberate.
Avoid enabling default features unnecessarily when a smaller feature set is sufficient.
Remove dependencies that are no longer used, provided doing so does not break workspace functionality.
6. Code Organization and Readability
Keep functions focused on one coherent responsibility.
Use descriptive, domain-appropriate names.
Avoid abbreviations unless they are universally understood within the domain.
Avoid vague names such as data, thing, manager, handler, processor, and utils when more specific names are available.
Keep related functionality together.
Avoid splitting code into excessive numbers of tiny functions that make control flow difficult to follow.
Avoid enormous files containing unrelated responsibilities.
Keep imports clean and organized.
Avoid unnecessary comments and documentation that repeat the implementation.
Document non-obvious invariants, design decisions, and surprising behavior.
Prefer self-documenting code, but do not sacrifice clarity to avoid comments.
7. Consistency
Follow the established formatting and naming conventions throughout the workspace.
Do not introduce competing approaches to solving the same problem without a compelling reason.
When modifying existing code, align with the surrounding code's conventions unless those conventions are demonstrably problematic.
If a better convention is established, apply it consistently where practical.
Avoid one-off utility functions when an equivalent shared implementation already exists.
Do not introduce new architectural or stylistic patterns for isolated convenience.

Consistency is more valuable than personal stylistic preference.

8. Testing and Correctness
Write tests for meaningful behavior, not implementation details.
Test edge cases, failure paths, and important invariants.
Avoid redundant tests that provide no additional confidence.
Prefer deterministic tests.
Do not introduce arbitrary sleeps, timing assumptions, or flaky synchronization.
Keep test fixtures and helpers simple.
Do not weaken or delete tests merely to make a change pass.
Do not mock functionality unnecessarily when a real implementation is inexpensive and deterministic.
Ensure tests actually verify the intended behavior.
Fix the underlying problem rather than masking failures.
9. Refactoring Existing Code

When touching existing code, actively look for opportunities to improve it.

Remove dead code encountered during modifications.
Eliminate redundant logic when its removal is safe and relevant.
Simplify unnecessarily complicated control flow.
Replace poor naming with clearer terminology when practical.
Consolidate duplicated logic when doing so improves maintainability.
Remove unnecessary dependencies and abstractions when their removal is within scope.
Do not preserve bad patterns simply because they already exist.
Do not undertake unrelated, sweeping rewrites under the guise of cleanup.
Preserve existing behavior unless the task explicitly requires changing it.

Refactoring should be incremental, deliberate, and verifiable.

10. Scope Discipline
Do not solve problems that were not requested unless they are directly related to the change.
Do not introduce unrelated features.
Do not rewrite working code merely to express a personal stylistic preference.
Do not change public APIs without understanding their consumers and compatibility implications.
Do not introduce unnecessary configuration, extensibility, or customization points.
Avoid broad formatting or structural changes that obscure the actual functional diff.
Keep changes focused and reviewable.
11. Verification

Before considering work complete:

Run cargo fmt --all -- --check.
Run cargo check --workspace.
Run cargo clippy --workspace --all-targets --all-features.
Run relevant tests with cargo test --workspace.
Verify that no new warnings have been introduced.
Verify that no unused dependencies, imports, or dead code have been introduced.
Inspect the final diff for accidental changes, unnecessary complexity, and unrelated modifications.

If a verification command cannot be run, explicitly state why. Never claim that code was verified when it was not.

Do not blindly suppress Clippy warnings. Understand the underlying issue before deciding whether an exception is justified.

12. The Boy Scout Rule

Always leave touched code better than you found it.

This does not mean undertaking unrestricted refactoring. It means that when working in an area, you should not knowingly introduce additional mess or preserve obvious, low-risk defects that are directly relevant to your change.

Prefer small, continuous improvements over occasional massive cleanup efforts.

13. Final Review Standard

Before submitting any change, ask yourself:

Is this the simplest reasonable implementation?
Did I introduce any unnecessary abstractions?
Did I introduce unnecessary allocations, clones, or conversions?
Are ownership and borrowing handled appropriately?
Are errors handled correctly and transparently?
Does this respect existing crate and module boundaries?
Could this functionality live somewhere more appropriate?
Did I duplicate existing functionality?
Did I introduce unnecessary dependencies?
Is the code easy to understand without additional explanation?
Are there any unnecessary lines, branches, types, or functions?
Did I preserve existing behavior outside the intended change?
Have I actually verified the changes?

If the answer to any of these reveals an avoidable quality issue, address it before considering the work complete.

The ultimate standard: Code should be as simple as possible, but no simpler than correctness, clarity, and maintainability allow.