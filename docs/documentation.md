Commenting & Documentation Policy

Comments and documentation must be written like they would be in a professionally maintained production codebase by an experienced engineer.

Default to no comments. Code should communicate its behavior through clear naming, structure, types, and implementation. Comments are reserved for information that cannot be reasonably expressed through the code itself.

1. Comment Sparingly
Do not comment obvious code.
Do not narrate what a function, statement, loop, or conditional does.
Do not restate function names, variable names, type names, or signatures.
Do not add comments merely to make code appear more documented.
Do not add introductory comments before every function or module.
Do not add comments to explain straightforward control flow.
Do not add comments that simply translate code into English.
Do not add comments solely because a function is non-trivial or lengthy.
Prefer improving the code over explaining unnecessarily complicated code.

If the implementation is self-explanatory, leave it uncommented.

2. Comments Must Convey Additional Information

A comment is justified only when it communicates information that would otherwise be difficult to discover from the implementation.

Appropriate subjects include:

Why an unusual implementation or design decision was necessary.
Non-obvious invariants and assumptions.
Constraints imposed by external systems, protocols, or dependencies.
Important ownership, concurrency, or synchronization requirements.
Subtle edge cases and failure conditions.
Safety invariants, particularly around unsafe code.
Non-obvious performance considerations.
Workarounds for known upstream bugs or platform limitations.

Comments should explain why, not what.

3. No AI-Generated Prose

Do not write comments in the characteristic explanatory style of an AI assistant.

Strictly avoid:

Verbose, essay-like comments.
Overly formal or artificially polished language.
Unnecessary introductory phrases.
Repeated explanations of the same concept.
Excessive use of adjectives and qualifiers.
Generic statements that add no implementation-specific information.
Comments that describe hypothetical scenarios without a concrete reason.
Unnecessary summaries at the beginning or end of functions.
Conversational language addressing the reader.
Teaching-oriented explanations of ordinary programming concepts.
Excessive use of words like "simply", "essentially", "notably", "importantly", "ensures", and "allows".

Do not write comments as though answering a question in a chat conversation. Write them as concise engineering notes embedded in source code.

4. Be Concise and Direct
Prefer one concise sentence over a paragraph.
Prefer a short phrase when a complete sentence is unnecessary.
Remove introductory filler.
Avoid repeating context already established by the surrounding code.
Avoid excessive punctuation and unnecessary emphasis.
Avoid elaborate prose and unnecessary terminology.
Do not use numbered lists or bullet lists inside ordinary source comments.
Do not add conversational sign-offs or summaries.

A comment should communicate its point and get out of the way.

5. Preserve Existing Documentation Style
Follow the established documentation style of the surrounding codebase.
Do not rewrite existing comments merely to match personal stylistic preferences.
Do not expand concise existing comments into verbose explanations.
When modifying a function, only update its documentation when the existing documentation becomes inaccurate or incomplete.
Remove obsolete comments when changing the behavior they describe.
Never leave comments that contradict the implementation.
6. Rust Documentation Standards
Use /// documentation comments for public API documentation where appropriate.
Use //! for meaningful module-level documentation, not generic module introductions.
Do not generate documentation for every private function or internal implementation detail.
Document public APIs in terms of their contracts, guarantees, constraints, and important failure conditions.
Document non-obvious panic conditions and safety requirements.
Use # Safety, # Errors, and # Panics sections when they convey relevant information.
Avoid redundant # Examples sections containing trivial usage.
Do not generate documentation that merely repeats the function signature.
Do not add documentation to private types and methods without a concrete maintenance benefit.
7. No Comment Padding

Never add comments to:

Fill visual space.
Divide ordinary functions into artificial sections.
Label obvious implementation steps.
Explain straightforward variable assignments.
Describe the expected output of obvious expressions.
Provide redundant summaries of function behavior.
Make a diff look more substantial.
Demonstrate that the code was understood.

Avoid decorative comment banners and excessive section dividers.

8. Final Comment Review

Before submitting changes:

Review every newly introduced comment.
Ask whether it communicates information unavailable from the code.
Remove comments that merely narrate implementation details.
Reduce unnecessarily verbose comments.
Verify that every comment is accurate and relevant.
Ensure documentation describes actual behavior rather than intended or assumed behavior.

A codebase should not become more verbose simply because an AI agent touched it. Comments are engineering artifacts, not a measure of code quality.