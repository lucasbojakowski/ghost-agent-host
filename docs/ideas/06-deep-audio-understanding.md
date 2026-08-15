# Idea 06 — Deep audio understanding beyond hand-engineered features

## Thesis

Deepen `ghost-audio` from deterministic signal analysis into a broader audio-understanding stack that can provide structured evidence such as likely source/instrument class, texture/timbre descriptors, spectral envelopes and potentially interpretable EQ-shape summaries.

The goal is not to replace deterministic DSP with a black box. It is to add complementary evidence that helps an agent understand **what kind of sound it is hearing** and which measurements are relevant.

## Current strength

The current analyzer already gives inspectable high-resolution evidence across:

- level/integrity;
- loudness/dynamics;
- spectrum/bands;
- resonances;
- stereo relationships;
- frame-level series where retained.

This is valuable because it is deterministic, cheap, local and easy to compare before/after.

The limitation is semantic: a model still has to infer whether a measured spectrum belongs to a vocal, snare, distorted bass, pad, full mix or another source before applying domain knowledge.

## Research branches

### Mel-spectrogram representation

Generate compact mel-spectrogram or log-mel representations suitable for:

- lightweight classifiers;
- embedding models;
- multimodal/audio-capable LLM inspection where supported;
- retrieval against known reference/source exemplars.

Keep the raw audio and deterministic measurements as authority for numerical claims. Learned models should add labels/probabilities, not rewrite measurements.

### Instrument / source detection

Research hierarchical labels rather than a single brittle classifier:

```text
source family
  vocal
  drums/percussion
  bass
  harmonic instrument
  full mix
  ambience/fx

optional subtype
  kick / snare / hats
  acoustic/electric bass
  lead/backing vocal
  piano/guitar/synth/etc.
```

Return confidence and competing hypotheses. `unknown` is a valid result.

### Timbral descriptors

Explore stable descriptors that may be more useful to an agent than a hard instrument label:

- noisy ↔ tonal;
- transient ↔ sustained;
- bright/dark;
- dense/sparse;
- harmonic/percussive balance;
- spectral envelope shape;
- dynamic stability;
- stereo width/coherence.

Many of these can be derived deterministically or with shallow learned models.

### EQ / spectral-envelope extraction

Research representations that capture the broad tonal shape without pretending to recover a plugin EQ curve from the output alone.

Potential outputs:

```text
spectral envelope
broad tilt
relative band deviations
persistent resonant regions
smoothed source profile
```

This could later support reference matching and before/after comparisons.

## LLM use

Using an LLM directly over mel/spectrogram images may be useful as an **experimental semantic annotator**, but it should not become the only analysis path.

Questions to test:

- Does a multimodal model classify source family better from a spectrogram plus deterministic features than from features alone?
- Does the label actually improve downstream tool decisions?
- Are results stable across gain, short excerpts and processing changes?
- Can we calibrate confidence and detect uncertainty?
- Is local inference practical enough for interactive use?

## Data strategy

Do not start by building a large proprietary training pipeline.

First create an eval dataset from:

- known isolated stems/instruments;
- synthetic fixtures;
- public/licensed material where appropriate;
- user-labeled local examples not uploaded by default.

Measure robustness to:

- gain changes;
- EQ/compression;
- short capture length;
- mono/stereo;
- sample-rate variation;
- layered/ambiguous sources.

## How this interacts with the product

Source understanding can improve:

- which context recipe is selected;
- which analysis features are emphasized;
- which processing vocabulary appears in the prompt;
- which references are considered relevant;
- whether a task is likely to need listening at all.

But it should remain evidence, not policy. A user selection such as `[Lead Vocal]` may be more reliable than automatic classification and should usually win.

## Risks

- instrument classification is less useful than it sounds when sources are layered or heavily processed;
- spectrogram LLM calls may add cost/latency without improving action quality;
- semantic labels can create false confidence;
- training/eval data can become a large side project disconnected from DAW interaction.

## Score

- FL leverage: **3/5**
- Unlocks: **3/5**
- Differentiation: **4/5**
- Learning: **5/5**
- Effort: **4/5**
- Uncertainty: **4/5**
- Priority score: **19/30**

## Recommendation

**Run as a parallel research track, not the main next app.** Start with a small benchmark comparing existing deterministic features, log-mel representations and one semantic classifier/LLM path. Promote only evidence that measurably improves downstream agent decisions.