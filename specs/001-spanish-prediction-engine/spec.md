# Feature Specification: Spanish Prediction Engine

**Feature Branch**: `001-spanish-prediction-engine`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "Build the core Spanish predictive typing engine for PrediType. Given the
text immediately before the caret and limited recent context (at most the current message, held in
memory only and discarded when the field changes or prediction is paused), return up to 3 ranked
next-word completions or brief continuations. The user, not the engine, decides whether to insert a
suggestion. Support Spanish accents, ñ, ¿ ¡, capitalization, punctuation, word boundaries, empty
context, deletion, and partially typed words. Include a lightweight baseline that runs locally on a
modest Windows CPU with no GPU and no internet. Do not use a database, RAG, a chatbot, remote APIs,
or an online LLM. Do not persist typed text or learn from user messages. Define measurable quality,
latency (p95 ≤ 100 ms), memory (≤ 100 MB), and correctness acceptance criteria using a repeatable
Spanish customer-service test corpus. Windows/Chrome integration and UI are out of scope. Focus on
product behavior and outcomes; choose the implementation technology during planning."

## Overview

The prediction engine is the shared core that both PrediType clients (Windows widget and Chrome
extension) will rely on. A client hands the engine the text before the caret; the engine returns
zero to three ranked suggestions describing exactly what would be inserted if the agent chose one.
The engine never inserts, sends, or stores anything itself.

Its direct consumers are the PrediType clients acting on behalf of a customer-service agent, and the
team that evaluates prediction quality and performance. In this spec, "agent" means the
customer-service person typing, and "client" means the integration that calls the engine.

## Clarifications

### Session 2026-10-16

- Q: Where should the Spanish text used to build the engine's vocabulary, and the separate
  evaluation corpus, come from? → A: Open-licensed general Spanish text for broad vocabulary, plus a
  team-written Colombian customer-service set for domain phrasing and held-out evaluation. No real
  customer data.
- Q: Which computer should be the official reference for the 100 ms and 100 MB targets? → A: A
  modest laptop: 4-core x86-64 CPU at ≤ 2.5 GHz base (e.g., Intel Core i5 8th–10th gen U-series),
  8 GB RAM, integrated graphics, SSD, Windows 10/11.
- Q: When the engine isn't confident about any suggestion, should it still return up to 3, or
  return fewer (even none)? → A: Return only suggestions above a minimum confidence, tuned so that at
  least 50% of shown first-ranked suggestions are correct; otherwise return fewer or none.
- Q: If both clients run at once, should the engine keep context for only the focused field or
  for several fields at the same time? → A: One active field at a time. A focus change from any
  client discards the previous context, and returning to a field starts fresh.
- Q: Should the quality targets block release if missed, or stay provisional until the first
  baseline is measured? → A: Provisional. The first baseline measurement is recorded, then the
  targets are confirmed or revised once through a documented spec amendment, after which they become
  hard gates.
- Q: Should the engine block offensive or vulgar words? → A: No. No blocklist is applied; vocabulary
  is whatever the approved sources contain. The exclusion of personal data stays.
- Q: How many letters must be typed before word completions are offered? → A: No fixed minimum.
  Completions may appear from the first letter, gated by the minimum-confidence rule (FR-002), and a
  completion must add at least 2 characters.
- Q: May a completion that only restores accents or `ñ` (for example `comunicacion` →
  `comunicación`, adding 0 characters) be offered despite the 2-character rule? → A: Yes. A completion
  qualifies if it adds at least 2 characters **or** restores missing accents or `ñ` in the partial
  word.
- Q: Must the empty-context openers pass the minimum-confidence rule? → A: Yes. There are no special
  cases; openers are returned only if they meet the cutoff, so zero openers is valid.
- Q: When the text ends in a complete word with no trailing space (for example `gracias`), may next
  words be offered? → A: No. Only longer completions of that word are offered; next words appear only
  after a word boundary.
- Q: How do "never suggest *tú* forms" and "complete other-variant words the agent starts typing"
  combine? → A: *Tú*-form and other-variant words are never proposed unprompted. They are offered as
  completions only when the typed partial word can no longer lead to an *usted*/Colombian form (for
  example `ayudart` → `ayudarte`).
- Q: Does a suggestion include a trailing space? → A: No. Suggestion text never ends with whitespace.
  Whether a space is added after acceptance is defined once, in a shared client interaction spec,
  and applied identically by both clients.
- Q: What happens if the language asset is missing, corrupt, or incompatible? → A: The engine refuses
  to start and reports an error with no typed text in it. It never runs on a partial asset. The
  client stays usable with prediction unavailable, and typing is unaffected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Complete a partially typed word (Priority: P1)

An agent has typed the beginning of a Spanish word. The engine offers up to three likely completions
of that word, spelled correctly with accents and `ñ`, so the agent can finish the word with a single
acceptance instead of typing every letter.

**Why this priority**: Word completion is the most frequent and most reliable form of help, and it
works even with very little context. On its own it delivers a usable MVP.

**Independent Test**: Feed the engine prefixes ending in partial words from the test corpus and
verify that the intended word appears in the returned suggestions, correctly spelled and cased.

**Acceptance Scenarios**:

1. **Given** the text `Buenos dí`, **When** a prediction is requested, **Then** `días` is among the
   suggestions, and applying it produces `Buenos días`.
2. **Given** the text `Le pedimos una disculpa por las molest`, **When** a prediction is requested,
   **Then** `molestias` is among the suggestions.
3. **Given** the text `Su contrase`, **When** a prediction is requested, **Then** `contraseña` is
   among the suggestions, spelled with `ñ`.
4. **Given** the text `Gracias por su comunicacion` typed without the accent and the caret directly
   after the word, **When** a prediction is requested, **Then** a suggestion may offer
   `comunicación`, and applying it replaces only the partial word `comunicacion`.

---

### User Story 2 - Predict the next word after a completed word (Priority: P1)

An agent has just finished a word (followed by a space or punctuation). The engine offers up to three
likely next words, based on the preceding text in the current message, so the agent can keep writing
common customer-service phrasing with fewer keystrokes.

**Why this priority**: Next-word prediction provides the largest keystroke savings in repetitive
customer-service writing and is the second half of the core value.

**Independent Test**: Feed the engine prefixes ending in a word boundary from the test corpus and
measure how often the actual next word appears in the suggestions.

**Acceptance Scenarios**:

1. **Given** the text `Muchas gracias por `, **When** a prediction is requested, **Then** the engine
   returns 1–3 plausible next words (for example `su`, `comunicarse`, `contactarnos`), most likely
   first.
2. **Given** the text `Hola, ` (ending with a space), **When** a prediction is requested, **Then**
   suggestions do not require an extra leading space.
3. **Given** the text `Hola,` (no trailing space), **When** a prediction is requested, **Then** each
   next-word suggestion indicates that a space precedes it, so applying it produces `Hola, <word>`.
4. **Given** the text `¿En qué puedo `, **When** a prediction is requested, **Then** `ayudarle`
   (formal *usted*) is among the suggestions and `ayudarte` is not.

---

### User Story 3 - Respect Spanish orthography, capitalization, and punctuation (Priority: P2)

Suggestions match the agent's writing: sentence-initial capitals after `.`, `?`, `!`, line breaks,
and opening `¿` / `¡`; all-caps when the agent is typing in all-caps; lowercase mid-sentence; and
standard capitalization for words that are always capitalized. The engine never alters text the
agent has already completed.

**Why this priority**: Incorrect casing or missing accents make suggestions useless or embarrassing
to accept; this story turns raw predictions into acceptable Spanish text.

**Independent Test**: Run the curated orthography test set (casing, accents, `ñ`, `¿`, `¡`,
punctuation boundaries) and verify 100% of expected outcomes.

**Acceptance Scenarios**:

1. **Given** the text `Su pedido fue enviado. `, **When** a prediction is requested, **Then** all
   suggestions start with a capital letter.
2. **Given** the text `¿` at the start of a message, **When** a prediction is requested, **Then**
   suggestions are capitalized question openers (for example `Cómo`, `En`, `Puedo`) with no leading
   space.
3. **Given** the text `BUE`, **When** a prediction is requested, **Then** the completion is offered in
   all-caps (for example `BUENOS`).
4. **Given** the text `Gracias, le` (mid-sentence), **When** a prediction is requested, **Then**
   completions are lowercase.
5. **Given** the text `Hola dias ` where `dias` is a completed earlier word, **When** a prediction is
   requested, **Then** no suggestion modifies `dias`; the engine only affects the trailing partial
   word or appends new text.

---

### User Story 4 - Transient, private context (Priority: P2)

The engine uses only the current message as context, holds it in memory only, and discards it when
the client signals a field change, a pause, or an exit. Its behavior never adapts to what the agent
has typed.

**Why this priority**: This is a constitutional requirement (Principles III and IV). It doesn't add
prediction value, but the engine can't ship without it.

**Independent Test**: Replay the whole corpus while monitoring disk writes and network activity;
repeat identical requests before and after many unrelated requests and compare outputs.

**Acceptance Scenarios**:

1. **Given** context from a previous field, **When** the client signals a field change and then
   requests a prediction for `Hol`, **Then** the result is identical to that of a freshly started
   engine given `Hol`.
2. **Given** the engine has served thousands of requests, **When** the same request is repeated,
   **Then** the result is identical to the first time (no learning from input).
3. **Given** a full corpus replay, **When** disk and network activity are monitored, **Then** no typed
   text is written to disk and no network connection is attempted.
4. **Given** the client signals pause, **When** the pause takes effect, **Then** all held context is
   discarded and the engine returns no suggestions until resumed.

---

### User Story 5 - Repeatable quality and performance evaluation (Priority: P2)

The team can run one repeatable evaluation that replays a versioned Spanish customer-service test
corpus through the engine and reports quality, correctness, latency, and memory against the
acceptance thresholds, including for the lightweight baseline.

**Why this priority**: The constitution requires evidence-based claims (Principle IX). Without this
evaluation, no claim about quality or performance can be accepted.

**Independent Test**: Run the evaluation twice on the same reference computer with the same engine
and corpus version, and confirm that quality metrics are identical and performance metrics fall
within a small, documented variance.

**Acceptance Scenarios**:

1. **Given** the corpus and engine versions, **When** the evaluation runs, **Then** it produces a
   report with every metric in the Success Criteria, the reference-computer description, and the
   corpus and engine versions.
2. **Given** a metric falls below its threshold, **When** the evaluation runs, **Then** the report
   marks that criterion as failed.

---

### User Story 6 - Brief multi-word continuations (Priority: P3)

When the preceding text strongly implies a common phrase, the engine may offer a short continuation
of up to four words (for example `¿En qué` → `puedo ayudarle`) as one of its three suggestions.

**Why this priority**: Continuations increase savings for formulaic phrases but are less reliable
than single words. The product is complete without them.

**Independent Test**: Measure how often accepted continuations match the corpus text in full, and
verify they never exceed four words or cross a sentence end.

**Acceptance Scenarios**:

1. **Given** the text `Quedamos atentos a `, **When** a prediction is requested, **Then** a
   continuation such as `cualquier inquietud` may be offered alongside single-word suggestions.
2. **Given** any continuation, **When** inspected, **Then** it contains at most four words and does
   not continue past sentence-ending punctuation.

### Edge Cases

- **Empty context** (start of message): the engine returns up to 3 common message openers (for
  example `Hola`, `Buenos`, `Estimado`), capitalized, but only those that meet the minimum-confidence
  rule. Zero openers is a valid result.
- **Deletion**: after characters are deleted, a new request reflects only the current text. No
  suggestion based on deleted text is returned, and the engine has no per-request memory that could
  make it stale.
- **Switching between clients or fields**: when focus moves from a Chrome field to a desktop field
  (or the reverse), the previous field's context is discarded. Returning to the earlier field starts
  with no retained context; only the text before the caret supplied in the new request is used.
- **Rapid successive requests**: each result is identifiable with the request it answers, so a client
  can discard results for superseded requests.
- **Token that is not a word** (number, email address, URL, code, or a token longer than 30
  characters): the engine returns no suggestions.
- **Non-Spanish or nonsensical text**: the engine returns zero or few suggestions and never errors.
- **Emoji, control characters, unusual Unicode, mixed scripts**: handled without failure. Unsupported
  characters yield no suggestions instead of an error.
- **Very long message**: only the most recent part of the current message (up to the documented
  context limit) is considered, and latency targets still hold.
- **Partial word already a complete word** (for example `gracias` with no trailing space): only
  longer completions of that word are offered (FR-008). Next words are offered only after a word
  boundary (FR-009). The engine never suggests the identical word as a completion.
- **Low confidence**: when no candidate meets the minimum confidence level, the engine returns no
  suggestions instead of weak guesses. This is a normal outcome, not an error.
- **Duplicate suggestions** differing only by case or accents: they are collapsed into one.
- **Very short completions**: a completion that would add fewer than 2 characters (for example
  `d` → `de`) is not offered, because accepting it saves no keystrokes. The exception is a completion
  that restores missing accents or `ñ` (for example `comunicacion` → `comunicación`, `ano` → `año`).
- **Missing, corrupt, or incompatible language asset**: the engine does not start and reports an
  error that contains no typed text. No suggestions are produced, and typing in the host application
  is unaffected (FR-038, FR-039).
- **Partial word containing a typo**: the engine is not a spelling corrector. It may return no
  suggestions instead of guessing broadly.

## Requirements *(mandatory)*

### Functional Requirements

**Input and output**

- **FR-001**: The engine MUST accept, per request, the text immediately before the caret, limited to
  the current message, and an identifier for the request.
- **FR-002**: The engine MUST return between 0 and 3 suggestions per request, ordered from most to
  least likely. It MUST return only suggestions that meet a minimum confidence level. When fewer
  candidates qualify, it returns fewer, and when none qualify, it returns none. The confidence level
  is a fixed, versioned part of the engine, tuned on data other than the held-out evaluation corpus
  to satisfy SC-014.
- **FR-003**: Each suggestion MUST state exactly what would change if applied: the text to insert,
  how many trailing characters of the current partial word it replaces (zero for a next word), and
  whether a leading space is required. Suggestion text MUST NOT begin or end with whitespace. Any
  space after an accepted suggestion is a client behavior, defined once in a shared client
  interaction spec and applied identically by both clients. Replacement counts MUST refer to the
  text exactly as the client sent it, before any internal normalization, so that decomposed
  accents (NFD) are counted correctly.
- **FR-004**: Applying any suggestion MUST change only the current trailing partial word and/or append
  text. All text before the current partial word MUST remain unchanged.
- **FR-005**: The engine MUST NOT insert, send, or submit text itself. It only returns suggestions,
  and the client and agent decide whether to apply them.
- **FR-006**: Each result MUST be associated with the request it answers.
- **FR-007**: Results MUST be deterministic: the same input and the same engine and asset versions
  MUST produce the same suggestions in the same order.

**Prediction behavior**

- **FR-008**: When the text ends inside a word, the engine MUST offer completions of that word,
  starting from the first typed letter. There is no fixed minimum number of typed letters. Which
  completions are returned is governed by the minimum-confidence rule in FR-002. Each completion MUST
  either add at least 2 characters to what the agent has typed, or restore missing accents or `ñ` in
  the partial word (FR-016). When the text ends inside a word, only completions of that word are
  offered, never next words.
- **FR-009**: When the text ends at a word boundary (whitespace or punctuation), the engine MUST
  offer next-word predictions informed by the preceding text of the current message.
- **FR-010**: The engine MAY offer brief continuations of up to four words. A continuation MUST NOT
  cross sentence-ending punctuation and counts toward the 3-suggestion limit.
- **FR-011**: When there is no context (empty input), the engine MUST offer common message openers as
  candidates, subject to the same minimum-confidence rule as all suggestions (FR-002). It returns up
  to 3 that qualify, possibly none.
- **FR-012**: The engine MUST return no suggestions when the current token is a number, email address,
  URL, code-like string, or longer than 30 characters.
- **FR-013**: The engine MUST NOT return duplicate suggestions that differ only in case or accents.
- **FR-014**: The engine MUST NOT suggest personal data such as names of real people, phone numbers,
  or email addresses. No offensive-word blocklist is applied (see Assumptions).

**Spanish language handling**

- **FR-015**: Suggestions MUST use standard Spanish spelling, including accented vowels (á, é, í, ó,
  ú, ü) and `ñ`.
- **FR-016**: Partial-word matching MUST tolerate missing accents in what the agent typed (for example
  `informacio` → `información`). The resulting suggestion replaces only the partial word.
- **FR-017**: Suggestions MUST be capitalized at sentence starts: at message start and after `.`, `?`,
  `!`, `…`, or a line break, including when an opening `¿` or `¡` precedes them.
- **FR-018**: When the partial word is typed entirely in capitals (at least two letters), completions
  MUST be in capitals. When only its first letter is capitalized, completions MUST keep that initial
  capital.
- **FR-019**: Words whose standard form is capitalized (proper nouns, acronyms) MUST keep their
  standard capitalization.
- **FR-020**: Mid-sentence suggestions (for example after a comma) MUST be lowercase unless FR-019
  applies.
- **FR-021**: The engine MUST NOT modify or propose corrections to any completed word before the
  current partial word.
- **FR-022**: Regional vocabulary and register MUST follow the documented **Colombian Spanish,
  formal *usted*** locale profile: suggestions use *usted* / *ustedes* forms (for example
  `ayudarle`, `le informamos`), Colombian vocabulary where it differs from other variants (for
  example `celular`, `computador`), and common Colombian customer-service phrasing (for example
  `con mucho gusto`, `a la orden`). *Tú*-form and other-variant words (for example `ayudarte`,
  `móvil`) MUST NOT be proposed as next words or continuations, nor as completions while the typed
  partial word could still lead to an *usted* or Colombian form. They MAY be offered as completions
  once the typed partial word can only lead to such words (for example `ayudart` → `ayudarte`). The
  engine MUST NOT change the register or vocabulary of text the agent has already typed.

**Privacy and context lifecycle**

- **FR-023**: The engine MUST hold context only in memory and MUST NOT write typed text, context,
  requests, or suggestions to disk, logs, or any persistent store.
- **FR-024**: The engine MUST hold context for at most one active field at a time, across all
  clients. It MUST discard all held context when any client signals a field change (including focus
  moving to a field in the other client), pause, or exit. After a field change, results MUST equal
  those of a freshly started engine, including when focus returns to a previously used field.
- **FR-025**: While paused, the engine MUST return no suggestions and retain no context.
- **FR-026**: The engine MUST NOT adapt, learn, or change its behavior based on any text it receives.
- **FR-027**: The engine MUST NOT make or require any network connection, and MUST NOT depend on a
  database, retrieval system, conversational assistant, or remote inference service.
- **FR-028**: All language assets the engine needs at runtime MUST be bundled or installed locally,
  and MUST NOT contain real customer conversations or personal data. Assets MUST be built only from
  (a) open-licensed general Spanish text whose license permits redistribution in the product, and
  (b) the team-written Colombian customer-service set. The source and license of each input MUST be
  documented.

**Robustness and performance**

- **FR-029**: The engine MUST handle any text input, including emoji, control characters, and mixed
  scripts, without failing. Unsupported input yields zero suggestions.
- **FR-030**: Producing a prediction MUST NOT block or delay the client's receipt of keyboard input.
  The engine MUST be usable without waiting on it in the typing path.
- **FR-031**: The engine MUST consider at most the most recent 1,000 characters of the current
  message. Longer input is truncated from the beginning, at a word boundary.
- **FR-032**: The engine MUST run on a modest Windows computer's CPU alone, with no GPU and no
  internet connection.

**Baseline and evaluation**

- **FR-033**: The feature MUST deliver a lightweight baseline predictor that satisfies all functional
  requirements and performance targets. Any later, more sophisticated approach MUST be measured
  against it.
- **FR-034**: The feature MUST deliver a versioned Colombian Spanish customer-service test corpus,
  written by the team for this purpose and free of real customer data. It MUST cover greetings, apologies,
  questions, order/shipping/billing/account vocabulary, closings, accents, `ñ`, `¿`, `¡`, and
  mixed punctuation.
- **FR-035**: The test corpus MUST be held out from any text used to build the engine's language
  assets.
- **FR-036**: The feature MUST deliver a curated correctness test set covering accents, `ñ`, `¿`,
  `¡`, capitalization, punctuation, word boundaries, empty context, deletion, partial words, and the
  edge cases listed above.
- **FR-037**: The feature MUST deliver a repeatable evaluation that reports every Success Criterion
  with pass/fail status, the reference-computer description, and the corpus, engine, and asset
  versions.

**Asset failure handling**

- **FR-038**: If the language asset is missing, unreadable, corrupt, or incompatible, the engine MUST
  refuse to start and report a descriptive error. The error MUST NOT contain typed text. The engine
  MUST NOT run on a partially loaded or unverified asset.
- **FR-039**: Each language asset MUST declare its version and the format version it uses. The engine
  MUST accept only asset formats it supports, and MUST report the incompatibility otherwise.

### Key Entities

- **Prediction Request**: The text before the caret (current message only), a request identifier,
  and an optional field-change or pause signal. Held only in memory.
- **Suggestion**: One candidate: text to insert, number of trailing partial-word characters it
  replaces, whether a leading space is needed, rank, and kind (word completion, next word, or
  continuation).
- **Prediction Result**: An ordered list of 0–3 suggestions tied to one request identifier.
- **Transient Context**: The in-memory view of the current message in the single active field
  (at most one exists at any time, across all clients). Discarded on field change, pause, or exit.
  Never persisted.
- **Language Assets**: Static, versioned, locally installed vocabulary and phrasing knowledge used to
  rank suggestions. Contain no user conversations.
- **Evaluation Corpus**: A versioned set of Spanish customer-service messages used only for
  measurement.
- **Correctness Test Set**: Curated input/expected-outcome cases for orthography and edge-case
  behavior.
- **Evaluation Report**: The output of one evaluation run: metric values, pass/fail per criterion,
  hardware description, and versions.

## Success Criteria *(mandatory)*

All criteria are measured by the repeatable evaluation (FR-037) on the reference computer (4-core
x86-64 CPU at ≤ 2.5 GHz base, 8 GB RAM, integrated graphics, SSD, Windows 10/11; see Assumptions), by replaying the evaluation corpus one keystroke at a time.

### Measurable Outcomes

**Quality**

SC-001 to SC-004 and SC-014 are **provisional targets**. The first baseline evaluation report is
recorded, and the targets are then confirmed or revised once through a documented amendment to this
spec that cites that report. After the amendment they are hard release gates and may only be lowered
through a further spec amendment with evidence. SC-005 and all Correctness, Performance, and Privacy
criteria are hard release gates from the start.

- **SC-001**: For partial words with at least 2 typed letters, the intended word appears among the
  suggestions in at least 70% of cases.
- **SC-002**: At word boundaries, the actual next word appears among the suggestions in at least 35%
  of cases.
- **SC-003**: A simulated agent who accepts a suggestion whenever it matches the intended text saves
  at least 20% of keystrokes across the corpus. Savings count characters not typed, minus one
  keystroke per acceptance.
- **SC-004**: When a correct suggestion is present, it is ranked first in at least 60% of cases.
- **SC-005**: 0 suggestions in the full corpus replay contain personal data or a misspelling,
  including a missing required accent or `ñ`.
- **SC-014**: Of all requests for which the engine returns at least one suggestion, the first-ranked
  suggestion matches the intended text in at least 50% of cases. This verifies the minimum-confidence
  rule in FR-002. SC-001 and SC-002 count requests with no suggestions as misses.

**Correctness**

- **SC-006**: 100% of cases in the correctness test set pass.
- **SC-007**: 100% of applied suggestions in the corpus replay leave all text before the current
  partial word unchanged.
- **SC-008**: Two evaluation runs with identical versions produce identical suggestions for every
  request.

**Performance**

- **SC-009**: p95 time from request to available result is at most 100 ms, and p99 at most 200 ms.
- **SC-010**: Working memory of the prediction engine stays at or below 100 MB throughout the corpus
  replay.
- **SC-011**: The engine is ready to serve its first suggestion within 3 seconds of being started.

**Privacy**

- **SC-012**: During a full corpus replay, 0 bytes of typed text are written to disk and 0 network
  connections are attempted.
- **SC-013**: After a field-change signal, 100% of follow-up requests return results identical to a
  freshly started engine.

## Assumptions

- **Reference computer** (agreed 2026-10-16): a Windows 10/11 x86-64 laptop with a 4-core CPU at
  no more than 2.5 GHz base clock (e.g., Intel Core i5 8th–10th gen U-series), 8 GB RAM, integrated
  graphics, and an SSD, measured on mains power with the default Windows power plan. The exact
  machine model used is recorded in each evaluation report.
- **Corpus size**: the evaluation corpus has at least 500 messages and at least 5,000 prediction
  points. It is drawn from the team-written Colombian customer-service set (see FR-028, FR-035) and
  reviewed for absence of personal data. Open-licensed general text is used only to build assets,
  never as the evaluation corpus.
- **Quality thresholds** (SC-001 to SC-004, SC-014) are initial estimates typical of a lightweight
  local predictor in a narrow domain. They are provisional until confirmed or revised once after the
  first baseline measurement (see Success Criteria).
- **Empty-context behavior**: offering message openers at the start of a message is assumed to be
  helpful. A client may choose not to display them.
- **Locale profile**: Colombian Spanish with formal *usted* (FR-022). The evaluation corpus and
  language assets reflect Colombian customer-service writing; other variants are not claimed as
  equally supported.
- **No offensive-word filtering**: by decision (2026-10-16), no blocklist is applied. Suggestions
  can include any word present in the approved sources (FR-028), including vulgar words found in
  open-licensed general text. Because suggestions only rank highly when they are frequent in context
  and pass the confidence rule, such words are expected to be rare in customer-service phrasing.
- **Context limit**: 1,000 characters is assumed to cover a typical customer-service message while
  keeping latency predictable.
- **Spelling tolerance** is limited to missing accents. General typo correction is out of scope
  because PrediType is not a grammar or spelling corrector.
- **Out of scope**: Windows widget and Chrome extension integration, any user interface, keyboard
  shortcuts, sensitive-field detection (a client responsibility), languages other than Spanish,
  personalization, and the transport between clients and the engine (decided during planning).
- **Dependencies**: the constitution (v1.0.0), in particular Principles I–V and IX.
