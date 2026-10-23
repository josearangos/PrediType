# Locale Profile: Colombian Spanish, formal *usted* (es-CO)

This is the locale profile required by spec FR-022. The engine, the language asset, the
team-written corpus and the correctness set all follow it.

## Register rules

1. **Address the customer as *usted*** (singular) or ***ustedes*** (plural). Verb forms, pronouns
   and possessives follow: `usted puede`, `le informamos`, `su pedido`, `ayudarle`, `comuníquese`.
2. **Never propose *tú*, *vos* or *vosotros* forms.** Words such as `tú`, `te`, `ti`, `contigo`,
   `tu`, `tus`, `puedes`, `tienes`, `quieres`, `ayudarte`, `vosotros`, `os`, `vuestro` carry the
   `register_other` flag in the asset (`data/lexicon/register-flags.txt`).
   - They are **never** offered as next words or continuations.
   - They are offered as completions only once the typed partial word can no longer lead to an
     *usted* or Colombian word (for example `ayudart` → `ayudarte`). The engine never changes text
     the agent already typed.
3. **Polite customer-service tone**: `con mucho gusto`, `a la orden`, `permítame un momento`,
   `quedo atento`/`quedo atenta`, `que tenga un feliz día`.
4. **Agent voice**: the engine predicts what an agent writes to a customer, not customer messages.

## Vocabulary differences

Colombian forms are preferred. Forms from other variants are flagged like *tú* forms.

| Colombia (preferred) | Other variants (flagged) | Meaning |
|----------------------|--------------------------|---------|
| `celular` | `móvil` | mobile phone |
| `computador` | `ordenador` | computer |
| `carro` | `coche` | car |
| `plata` (informal) / `dinero` | `pasta` | money |
| `parqueadero` | `aparcamiento` | parking lot |
| `cédula` | `DNI` | national ID |
| `jugo` | `zumo` | juice |
| `tiquete` | `billete` (ticket sense) | ticket |
| `guayabo` (informal) | `resaca` | hangover |
| `regáleme` (polite request) | — | "please give me" |
| `le colaboro` | — | "I'll help you" |

Only the clearly regional forms in the flagged column are in `register-flags.txt`. Words like
`dinero` or `billete` are standard and stay unflagged.

## Customer-service phrases

| Situation | Phrases |
|-----------|---------|
| Greeting | `Buenos días`, `Buenas tardes`, `Hola, ¿cómo está?`, `Con mucho gusto le atiendo` |
| Offering help | `¿En qué le puedo ayudar?`, `¿En qué le puedo colaborar?`, `Con gusto le colaboro` |
| Waiting | `Permítame un momento, por favor`, `Gracias por su paciencia` |
| Apology | `Qué pena con usted`, `Le pedimos una disculpa por las molestias` |
| Confirmation | `Con mucho gusto`, `A la orden`, `Listo` |
| Closing | `Quedamos atentos a cualquier inquietud`, `Que tenga un feliz día` |

## Out of scope

Other Spanish variants are not claimed as equally supported (spec Assumptions).
