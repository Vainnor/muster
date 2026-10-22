---
paths:
  - "docs/**"
  - "*.md"
  - "web/src/**"
---
# Writing Rules (not loaded at startup; read in full before writing docs, site copy, a commit body or PR text)

Prose a competent engineer would write: specific, direct, American English.

## Structure
- Lead with the point, then the detail.
- Specific over abstract: a number, a file path, a named function, a concrete failing case.
- Active voice; name the actor ("the store backs up the bad file", not "the file is backed up").
- Vary sentence length. Uniform medium-length sentences read as machine text.
- Site copy: say what the app does in plain words a non-technical user understands. No hype.

## Banned vocabulary
Cut and replace unless there is a technical reason:
delve, showcase, underscore, foster, garner, enhance, navigate (figurative), embark, harness,
empower, leverage, utilize, facilitate, transform, revolutionize, unlock (figurative), elevate,
streamline, supercharge; robust, vibrant, intricate, comprehensive, holistic, seamless(ly),
bespoke, cutting-edge, paramount, pivotal, crucial, vital, dynamic (figurative), groundbreaking,
myriad, plethora; synergy, paradigm, tapestry, realm, landscape (figurative), journey
(figurative), testament, game-changer; arguably, potentially, "it's worth noting";
furthermore, moreover.

## Banned patterns
- "It's not X, it's Y" / "not because X but because Y". State the thing.
- Rhetorical question then answer ("The result? Speed."). Keep the answer.
- Reflexive rule of three ("fast, simple, private") more than once per page.
- "Whether you're X or Y", "Imagine…", "In today's world…", "Designed to…" (say what it does).
- Emoji in docs, headings or commits.

## Claims
Every factual claim (a size, a speed, "no network calls") is true of the code at the commit
that carries it. Check it, or leave it out.
