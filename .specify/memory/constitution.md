# PrediType Constitution

## Project Purpose

PrediType is a lightweight, Spanish-first predictive typing tool for agents who write
customer-service messages across many applications. It suggests the next word or a short
continuation while the user types. The user remains in control and explicitly chooses whether to
insert a suggestion.

PrediType has two user-facing clients:

1. A floating Windows widget intended to work in supported desktop applications.
2. A Google Chrome extension intended to work in supported web text fields.

Both clients use a shared, local-first prediction capability wherever practical. PrediType is a
predictive keyboard aid, not a chatbot, message writer, grammar-correction suite, or
customer-service knowledge system.

## Core Principles

### I. Prediction Only; The User Is in Control

- The product MUST suggest next-word completions or short text continuations.
- The product MUST NOT send messages, submit forms, or replace user text without an explicit user
  action.
- Suggestions MUST be dismissible without changing the user's text.
- The user MUST be able to continue typing normally when no suggestion is shown or when PrediType
  is disabled.
- The initial product MUST NOT attempt to answer questions, generate full replies from business
  knowledge, or behave as a conversational assistant.

**Rationale**: Agents are accountable for every message they send; the tool must accelerate their
writing without ever acting on their behalf.

### II. Spanish-First Language Quality

- Spanish is the primary and required language for the initial release.
- The system MUST handle Spanish characters and orthography, including `ñ`, accented vowels,
  opening punctuation (`¿`, `¡`), capitalization, and common punctuation.
- Prediction behavior MUST be evaluated using Spanish-language examples that resemble real
  customer-service writing.
- The initial release MUST NOT claim equivalent support for other languages.
- Spanish locale assumptions, including regional vocabulary, MUST be documented and MUST NOT
  silently alter a user's text.

**Rationale**: Prediction quality in the target language is the product's core value; generic or
English-centric behavior would fail the primary users.

### III. Local-First and Offline by Default

- Prediction MUST work without an internet connection after installation.
- Typed text and prediction context MUST be processed locally.
- Runtime prediction MUST NOT call external APIs or transmit text to a remote service.
- The initial release MUST NOT use retrieval-augmented generation (RAG), a remote LLM, a chatbot,
  or a cloud inference dependency.
- Any language model, lexicon, or prediction assets required at runtime MUST be packaged locally
  or be explicitly installed by the user or administrator.
- Network access MUST NOT be required to produce suggestions.

**Rationale**: Customer-service text frequently contains personal data; keeping processing local
eliminates a whole class of privacy, compliance, and availability risks.

### IV. No Database and No Persistent Capture of Typed Text

- The prediction feature MUST NOT require or connect to a database.
- The initial release MUST NOT store typed text, surrounding message context, accepted
  suggestions, dismissed suggestions, or conversation contents on disk.
- Raw keystrokes and text context MUST NOT be logged, sent as telemetry, or retained for model
  training.
- Configuration preferences MAY be stored in a small local configuration file if persistence is
  needed; such configuration MUST NOT contain typed content.
- Static model assets and bundled dictionaries are allowed. They are not a database and MUST NOT
  contain users' conversations.
- Any future personalization or learning-from-user-text feature is out of scope unless separately
  specified, reviewed for privacy, and explicitly approved by the user.

**Rationale**: Text that is never persisted cannot be leaked, subpoenaed, or misused; a
stateless design is also simpler to build and verify.

### V. Lightweight Performance on Modest Windows PCs

- The application MUST be designed for Windows computers with modest CPU and memory resources and
  no dedicated GPU.
- Prediction MUST NOT block the thread responsible for receiving keyboard input or interfere with
  normal typing.
- The prediction engine MUST be asynchronous or otherwise isolated from the typing event path.
- Performance MUST be measured on an explicitly documented reference computer before release.
- Initial targets: p95 time from a prediction request to available suggestions of no more than
  100 ms on the agreed reference computer, and no more than 100 MB working memory for the
  prediction runtime, excluding the Chrome browser and operating system. These targets MUST be
  validated, and any deviation MUST be documented with evidence.
- Resource use, startup time, and typing responsiveness MUST be treated as release criteria, not
  optional optimizations.

**Rationale**: A suggestion that arrives late or slows typing is worse than no suggestion; target
users often work on constrained corporate hardware.

### VI. Two Clients, Clear Boundaries

- The Windows widget and Chrome extension MUST remain separate integration clients with clearly
  defined boundaries.
- Prediction rules and behavior SHOULD be shared through a common engine or a versioned local
  protocol rather than implemented inconsistently in each client. Any client-specific divergence
  MUST be justified in the relevant plan.
- The Windows client MUST focus on desktop text-entry integration and a floating suggestion
  surface.
- The Chrome extension MUST focus on browser text fields and supported editable elements.
- The architecture MUST NOT promise compatibility with every application or custom editor.
  Supported and unsupported environments MUST be documented and tested.
- The implementation plan, not this constitution, selects concrete technologies and IPC details
  after comparing viable options.

**Rationale**: Separate clients keep platform-specific complexity contained, while a shared engine
keeps suggestions consistent for users who move between desktop and browser.

### VII. Safe and Respectful Input Handling

- PrediType MUST provide an obvious way to pause or disable prediction and a reliable way to exit
  the application.
- PrediType MUST NOT make suggestions in password fields and other controls identified as
  sensitive or non-text-entry controls.
- The Chrome extension MUST request the minimum permissions needed for its documented behavior.
- The Windows client MUST capture only the input context needed for transient prediction and MUST
  NOT behave as a general-purpose keylogger.
- Input capture MUST stop when the product is paused, disabled, or exited.
- Suggestions MUST never silently overwrite existing text. Insertion MUST be limited to the
  intended completion and MUST preserve unrelated text as far as the host application's editing
  model allows.
- Known limitations involving elevated applications, remote desktops, protected controls,
  accessibility tools, or custom editors MUST be documented.

**Rationale**: Input-observing software carries keylogger-level trust; it must earn that trust
through minimal capture, clear controls, and non-destructive editing.

### VIII. Predictable and Non-Intrusive Interaction

- Suggestions MUST be visually distinct from text the user has already written.
- The user MUST be able to accept, dismiss, and navigate suggestions through documented keyboard
  interactions.
- When no valid suggestion is available, the interface MUST remain unobtrusive.
- The product MUST NOT steal focus from the active writing field merely to display a suggestion.
- Keyboard shortcuts MUST be configurable or chosen to minimize conflicts with common browser and
  desktop workflows.
- Acceptance and dismissal behavior MUST be consistent across clients wherever the host
  application permits it.

**Rationale**: Agents type quickly and from muscle memory; surprising UI or stolen focus disrupts
their work more than it helps.

### IX. Testability and Evidence-Based Quality

- Requirements MUST be testable and acceptance criteria MUST be observable.
- The prediction engine MUST have automated tests for Spanish text, prefixes, punctuation,
  capitalization, empty context, deletion, and boundary conditions.
- Windows and Chrome integrations MUST have separate tests as well as end-to-end compatibility
  checks.
- Performance claims MUST be supported by repeatable measurements on documented hardware.
- The project MUST distinguish verified compatibility from assumed or untested compatibility.
- Failures that can corrupt text, swallow input, leak captured text, or interfere with normal
  typing are release-blocking defects.

**Rationale**: Compatibility and performance claims for input tools are easy to overstate;
evidence keeps the product honest and the users safe.

### X. Spec-Driven Development and Maintainability

- Microsoft Spec Kit MUST be used to define feature behavior before implementation.
- Specifications MUST describe user-visible behavior, constraints, and verifiable outcomes before
  technical plans select implementation details.
- Each feature MUST follow a dependency-aware workflow: specify, clarify where needed, plan,
  checklist, tasks, analyze, implement, and converge.
- Changes to architecture or user-visible behavior MUST be reflected in the relevant
  specification and tests.
- Prefer the simplest design that satisfies the product constraints. Unnecessary services,
  frameworks, background dependencies, and abstractions MUST be avoided or justified in the plan.
- Modules MUST be small, typed where the language supports it, documented, and independently
  testable.

**Rationale**: Writing behavior down before building it keeps two clients and a shared engine
aligned, and simplicity keeps the runtime light.

## Product Boundaries and Non-Goals

The initial release does NOT include:

- RAG, vector databases, a knowledge base, or retrieval from company documents.
- Cloud inference, remote LLM APIs, or a required online account.
- A chatbot or automatic generation of complete customer-service replies.
- Automatic message sending, form submission, or autonomous actions in other applications.
- Conversation history, user-text logging, telemetry based on typed content, or model training on
  agent messages.
- A required database of any kind.
- Supervisor monitoring, agent scoring, or analytics about individual agents.
- A promise that the product works in every Windows application, every browser editor, or every
  remote desktop environment.

## Required Definition of Done

A feature is complete only when:

1. Its acceptance criteria are demonstrably satisfied.
2. Automated tests pass, including relevant regression tests.
3. The feature does not violate the offline, privacy, no-database, or user-control principles.
4. Keyboard input remains responsive and text is not altered without explicit user acceptance.
5. Performance and resource impact are measured when relevant.
6. User-facing limitations and setup instructions are documented.
7. The feature's Spec Kit artifacts accurately reflect the delivered behavior.

## Governance

- This constitution supersedes other project practices and guidance where they conflict.
- Amendments MUST be made through the Spec Kit constitution workflow (`/speckit-constitution`),
  with a Sync Impact Report describing the change.
- Any proposed exception to offline operation, no-database behavior, data retention, or explicit
  user control requires an explicit constitution amendment before implementation.
- If a feature request conflicts with this constitution, the conflict MUST be raised before
  implementation rather than silently ignored.
- **Versioning policy** (semantic versioning):
  - MAJOR: removal or backward-incompatible redefinition of a principle or governance rule.
  - MINOR: a new principle or section, or materially expanded guidance.
  - PATCH: clarifications, wording, or typo fixes with no semantic change.
- **Compliance review**: every implementation plan MUST include a Constitution Check against
  Principles I–X; `/speckit-analyze` MUST flag constitution conflicts as critical; reviews MUST
  confirm the Required Definition of Done before a feature is accepted. Unjustified complexity or
  unresolved violations block merge.

**Version**: 1.0.0 | **Ratified**: 2026-10-16 | **Last Amended**: 2026-10-16
