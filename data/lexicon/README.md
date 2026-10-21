# Lexicon lists

- `register-flags.txt`: *tú*-form and other-variant words (FR-022). They get the
  `register_other` flag and are never proposed as next words, and only as completions once no
  *usted*/Colombian word still matches the typed prefix.
- `domain-words.txt`: legitimate customer-service terms that aren't in the `es_CO` dictionary.
  Every word in the asset must be in the dictionary or in this list (SC-005).

One word per line, UTF-8. Lines starting with `#` are comments.
