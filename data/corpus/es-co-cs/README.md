# Colombian Customer-Service Corpus (`es-co-cs`)

This is the team-written domain corpus (spec FR-034, research R3, task T013). It's used to:

- build domain n-grams (`build` split);
- tune confidence cutoffs (`dev` split);
- evaluate the engine (`test` split, held out and never used to build assets).

## Files

| File | Format |
|------|--------|
| `messages.tsv` | UTF-8, header `category<TAB>text`, one message per line, no tabs or line breaks inside a message |
| `splits.tsv` | `message_sha256<TAB>split`. Generated **once** by `predi-build split` and never edited by hand (T014) |

## Writing rules

1. **Register: formal *usted***. Write `le colaboro`, `¿en qué le puedo ayudar?`, `ustedes`. Never use
   `tú`, `vos`, or `vosotros`.
2. **Colombian phrasing**. Use natural contact-center Spanish from Colombia, for example `con mucho
   gusto`, `a la orden`, `qué pena con usted`, `le colaboro`, `regáleme`, `permítame un momento`,
   `quedo atento`, `que tenga un feliz día`. Use `celular`, not `móvil`, and `computador`, not
   `ordenador`.
3. **No personal data, ever**. Use these placeholders instead:

   | Placeholder | Meaning |
   |-------------|---------|
   | `{NOMBRE}` | A person's name |
   | `{NUMERO_PEDIDO}` | Order number |
   | `{NUMERO_GUIA}` | Shipping tracking number |
   | `{NUMERO_FACTURA}` | Invoice number |
   | `{TELEFONO}` | Phone number |
   | `{CORREO}` | Email address |
   | `{DIRECCION}` | Street address |
   | `{FECHA}` | Date |
   | `{VALOR}` | Money amount |

   Write no real names, numbers, emails, addresses, or company names. Generic city names (Bogotá,
   Medellín, Cali) are allowed.
4. **Correct orthography**. Use accents, `ñ`, opening `¿` and `¡`, and standard punctuation. The
   corpus is the reference for spelling quality.
5. **Agent voice**. Messages are what an agent writes to a customer, not customer messages.
6. **Variety**. Avoid near-duplicates. Mix short (3–6 words) and long (2–3 sentences) messages.
7. **One message per line**, 1–3 sentences.

## Categories and minimums

The total is at least 1,500 messages.

| Category | Content | Minimum |
|----------|---------|---------|
| `saludo` | Greetings and openings | 120 |
| `disculpa` | Apologies and empathy | 120 |
| `pregunta` | Questions to the customer | 120 |
| `pedido` | Orders and purchases | 120 |
| `envio` | Shipping and delivery | 120 |
| `facturacion` | Billing, payments, refunds | 120 |
| `cuenta` | Account, access, passwords, personal data updates | 120 |
| `cierre` | Closings and farewells | 120 |
| `otro` | Hold messages, transfers, policies, general information | — |

## Review checklist (before freezing the splits)

- [ ] No real personal data anywhere. Every identifier is a placeholder.
- [ ] Every message uses *usted* and Colombian vocabulary.
- [ ] Spelling and accents are correct.
- [ ] No exact or trivial duplicates.
- [ ] Category minimums are met.

## Provenance

The first draft was prepared with AI assistance on 2026-10-16 and needs the human review above
before `predi-build split` is run.
