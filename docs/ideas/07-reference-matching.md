# Idea 07 — Structured reference matching

## Thesis

Build a specialized reference system that turns a reference track or source into **structured, comparable evidence** rather than treating reference matching as a vague prompt such as "make this sound like X."

The system should compare target and reference through the same deterministic analysis pipeline and expose musically useful deltas to the agent.

## Why this is different from generic audio analysis

A single analysis answers:

> What is measurable about this sound?

Reference matching answers:

> How does this sound differ from the chosen reference under a comparable observation?

That changes the context shape from two independent blobs into a deliberate comparison.

## Proposed data model

Conceptually:

```text
ReferenceObservation
  reference_id
  source metadata
  analysis bundle
  optional semantic labels

ReferenceDelta
  loudness delta
  true-peak/headroom delta
  spectral tilt delta
  band-balance deltas
  spectral-envelope distance
  resonance differences
  transient/dynamic differences
  stereo differences
  confidence / comparability flags
```

Comparability matters. A four-second vocal phrase should not be blindly compared with a mastered full-song chorus and presented as a precise target.

## Normalization questions

Reference comparisons need explicit policy around:

- loudness normalization before tonal comparison;
- mono/stereo compatibility;
- sample-rate differences;
- time-window length;
- silence/trailing tails;
- source-class compatibility;
- whether dynamics should be compared before or after gain alignment.

These policies should be inspectable and versioned. An agent should know when a delta is weak evidence.

## Product interaction

Possible UX:

```text
[Lead Vocal] [bars 49–57] [Reference: Artist / Song / vocal excerpt]

"move this closer to the reference, but keep it less bright"
```

Ghost can provide:

```text
current target observation
reference observation
structured delta
user's explicit exceptions/preferences
available DAW actions
```

This is much more useful than making the model infer all differences from two long analysis JSON objects.

## Reference library

A future local reference library could store:

- user-selected songs/stems/excerpts;
- tagged source type/instrument/role;
- analysis version and extracted features;
- loudness-normalized derived representations;
- optional notes such as genre, mix role or why the user likes it;
- multiple regions from one track.

The system should work locally by default. Do not assume references need to be uploaded.

## Retrieval direction

Once enough references exist, Ghost could retrieve relevant examples based on:

- user selection/role (`lead vocal`, `kick`, `mix bus`);
- learned embeddings;
- deterministic feature similarity;
- tags/notes;
- project style context.

This should remain secondary to explicit user-chosen references. Automatic retrieval can suggest; the user can decide.

## Evaluation

A first benchmark can use known pairs with controlled transformations:

- same source with EQ tilt;
- same source with compression;
- level-only differences;
- stereo-width changes;
- deliberately mismatched source classes.

Test whether the system correctly identifies the transformation direction and whether the agent chooses more coherent DAW actions with `ReferenceDelta` than with raw analysis alone.

## Relationship to closed-loop evaluation

Reference matching combines naturally with [08-closed-loop-audio-evaluation.md](08-closed-loop-audio-evaluation.md):

```text
target A vs reference
      ↓
agent acts
      ↓
target B vs reference
      ↓
delta improvement / regression
```

That produces a much stronger verification signal than "the parameter changed."

## Risks

- references can encourage destructive over-matching when arrangement/source role differs;
- feature distance can look scientific while encoding poor perceptual assumptions;
- copyrighted commercial references need careful local-only handling and should not become training data by default;
- automatic source matching may distract from explicit producer taste.

## Score

- FL leverage: **4/5**
- Unlocks: **4/5**
- Differentiation: **5/5**
- Learning: **5/5**
- Effort: **4/5**
- Uncertainty: **4/5**
- Priority score: **20/30**

## Recommendation

**Good parallel audio-intelligence project after the general FL agent is underway.** Start with deterministic A/B deltas and controlled fixtures before embeddings, large reference libraries or automatic retrieval.