# AI-writing tells and editing checklist

This is an editing rubric, not an authorship detector. No word, punctuation
mark, sentence pattern, or detector score proves that AI wrote a document.
Human writers use every feature below, models and genres differ, and editing
changes the signal. OpenAI withdrew its own classifier for low accuracy, and
independent evaluations have found large domain effects, evasion failures, and
false positives. Use these patterns to find bland or mechanical prose, not to
accuse an author.

## Sources and limits

- Pew Research Center's analysis of 490,000 English web pages found population-
  level increases in em dashes, Oxford commas, negative parallelism, and a set
  of AI-associated words. Pew explicitly warns that these features do not
  identify individual documents: <https://www.pewresearch.org/data-labs/2026/08/20/how-much-of-the-internet-is-written-with-ai/>
- Juzek and Ward identify 21 words overrepresented in recent scientific text,
  including `delve`, `intricate`, and `underscore`, but do not present any one
  word as proof of authorship: <https://arxiv.org/abs/2412.11385>
- Zanotto and Aroyehun compare 11 models across eight domains. Human texts were
  simpler syntactically and more semantically varied on average, while newer
  model outputs showed homogenization: <https://aclanthology.org/2025.emnlp-main.1163/>
- Tufts, Zhao, and Li found that text detectors can perform poorly on unseen
  models, domains, and moderate adversarial prompts; true-positive rate at a
  1% false-positive rate reached 0% in some settings:
  <https://aclanthology.org/2025.naacl-findings.271/>
- Stanford HAI reported that seven detectors falsely classified many essays by
  non-native English writers, illustrating the harm of treating style as
  provenance: <https://hai.stanford.edu/news/ai-detectors-biased-against-non-native-english-writers>
- OpenAI withdrew its 2023 classifier because of low accuracy. Its published
  evaluation found 26% true positives and 9% false positives:
  <https://openai.com/index/new-ai-classifier-for-indicating-ai-written-text/>

## Lexical tells

Look for clusters, not isolated words.

- Inflated importance: `crucial`, `pivotal`, `profound`, `significant`,
  `transformative`, `groundbreaking`, `remarkable`, `testament`.
- Stock academic verbs: `delve`, `underscore`, `highlight`, `showcase`,
  `illuminate`, `foster`, `enhance`, `leverage`, `navigate`.
- Decorative abstraction: `landscape`, `realm`, `tapestry`, `interplay`,
  `ecosystem`, `journey`, `paradigm`, `multifaceted`, `nuanced`, `intricate`.
- Generic approval: `robust`, `seamless`, `valuable`, `comprehensive`, `vibrant`,
  `meticulous`, `compelling`, `promising`.
- Empty intensifiers: `deeply`, `highly`, `increasingly`, `fundamentally`,
  `particularly`, `notably`, `remarkably` when no measurement follows.
- Canned transitions: `Additionally`, `Moreover`, `Furthermore`, `Consequently`,
  `In today's rapidly evolving`, `It is worth noting`, `It is important to
  note`, `In conclusion`.
- Vague attribution: `experts say`, `research suggests`, `it is widely
  recognized`, or `many believe` without a named source.

## Sentence-level tells

- Negative parallelism: `not only X, but Y`, `not merely X; it is Y`, or
  repeated `This is not X. It is Y.` constructions.
- Repeated rhetorical pivots: `The question is not ...`; `The real issue is
  ...`; `This distinction matters because ...`.
- Habitual triads: three adjectives, three nouns, or three clauses in sentence
  after sentence.
- Explanatory tails that merely restate the clause before them, often beginning
  with `thereby`, `thus`, `therefore`, `ensuring`, `highlighting`, or
  `underscoring`.
- Long chains of abstract nouns where a subject and verb would be clearer.
- Similar sentence lengths and syntactic shapes across a paragraph.
- Excessive em dashes, semicolons, parenthetical qualifications, or Oxford
  commas as a house style rather than a local choice.
- Correct but over-smoothed prose with no abrupt emphasis, concrete aside,
  memorable phrasing, or authorial preference.

## Paragraph and document-level tells

- Every paragraph opens with a topic sentence and closes with a summary.
- The introduction announces a roadmap that the headings already provide.
- The conclusion repeats the abstract and introduction without adding judgment.
- Many short sections with generic paired headings such as strengths/weaknesses,
  opportunities/challenges, or present/future.
- Repeated numbered frameworks whose categories are tidy but not analytically
  necessary.
- Mechanically balanced treatment of every advantage with a disadvantage.
- The same thesis paraphrased several times to create length.
- Excessive headings, bullets, bold labels, and colon-led definitions.
- Broad claims before specific evidence, followed by a citation that supports
  only part of the claim.
- Examples that could apply to any project rather than the artifact at hand.
- Missing negative results, uncertainty, trade-offs, provenance, or named
  responsibility.
- Fabricated, mismatched, inaccessible, or suspiciously perfect citations.

## Technical-paper checks

- Replace `substantial`, `meaningful`, `appropriate`, and `significant` with a
  measurement or delete them.
- State the exact environment and benchmark boundary.
- Separate observed behavior from inference and aspiration.
- Preserve failures and counterexamples instead of smoothing the narrative.
- Name who made a decision and who supplied the evidence.
- Prefer one concrete result over a paragraph of capability adjectives.
- Check every compatibility claim against a test or ledger entry.
- Do not let implementation, test authorship, and success judgment collapse
  into one unqualified claim.

## Audit of the Ginux report before tightening

| Pattern | Finding | Edit |
|---|---|---|
| AI-associated vocabulary | Low. None of `delve`, `intricate`, `tapestry`, `landscape`, or `underscore` appeared. | Preserve the ordinary vocabulary; do not perform cosmetic synonym replacement. |
| Negative parallelism | Repeated `not X but Y`, `not merely`, and `not only` constructions. | Keep the strongest contrast once and rewrite or delete the rest. |
| Repeated thesis | The construction-versus-assurance distinction appeared in the abstract, introduction, scope, findings, production discussion, and conclusion. | State it in the abstract, demonstrate it in results, and make one final judgment. |
| Framework symmetry | Three capability levels, four contributions, four cost classes, and several balanced lists appeared close together. | Retain only categories used later in the analysis. |
| Over-sectioning | Fourteen numbered sections plus numerous subsections and two appendices made a case study read like a generated outline. | Collapse to six sections and move publication checklists out of the article. |
| Uniform cadence | Most paragraphs used similarly long, carefully qualified sentences. | Mix direct short findings with longer technical explanation. |
| Metadiscourse | Phrases such as `This distinction matters`, `The correct conclusion`, and `It is therefore fair to say` told the reader how to interpret adjacent prose. | Make the underlying claim directly. |
| Repetition words | `evidence`, `therefore`, `the experiment`, and `the same` recurred heavily. | Remove duplicate claims and use concrete nouns where repetition remains. |
| Specificity and negative results | Strong. Exact commits, timings, syscall counts, costs, failures, and limitations resist generic AI prose. | Preserve these details during compression. |
| Citations | Strong after primary PHP sources were added; no invented sources found. | Keep citations adjacent to the exact supported claim. |

## Audit after the six-page rewrite

The audit was run only after the substantive cut from twelve pages to six. The
shortened manuscript retained the fixed commit, measurements, architecture,
negative lifecycle result, security discussion, and limitations. It removed
the milestone table, two appendices, repeated contribution and cost frameworks,
and eight top-level sections.

The second pass found no concentration of the lexical markers above. It did
find several patterned contrasts (`less X than Y`, `not X but Y`, and repeated
sentences explaining what a result did *not* establish). Those lines were
rewritten as direct claims. Canned interpretive phrases and repeated thesis
statements had already disappeared during the structural cut. Necessary
technical negation remains where it carries information, such as explaining
that zero `Missing` syscall entries does not mean complete implementations.

The finished prose deliberately retains the author's specific judgment,
including the joke about the project's name, the description of per-line cost
ratios as curiosities, and the conclusion that production use would be
reckless. Provenance is explicit: the report states that Codex wrote much of
the code, tests, documentation, and draft prose.

The later thesis rewrite was checked again after its substantive edits. It
retains one deliberate negative contrast in the opening section title---the
kernel is Linux-compatible but is not Linux---because that distinction is the
paper's subject, not a stylistic flourish. Repeated uses of `the same` in the
architecture and security sections were replaced with concrete subjects. The
rewrite continues to disclose AI involvement in both implementation and draft
prose.

## Editing rule

The goal is not to disguise provenance. The report openly studies an AI-built
artifact and should disclose how its prose was produced. The goal is to make
the author's argument sharper: fewer prefabricated transitions, less symmetry,
less repetition, more concrete evidence, and clear human judgment.
