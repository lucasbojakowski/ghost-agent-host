# Idea 12 — Plugin semantic profiles

## Thesis

Build an app-level knowledge layer that describes the semantic control surface of known third-party plugins without contaminating the raw DAW adapter.

The FL adapter should continue to expose exactly what Gopher exposes. A plugin profile should answer a different question:

> Given this concrete plugin/version, what do its published parameters mean and how can an agent manipulate them coherently?

## Motivation

The vertical-slice experiments showed the core problem clearly:

- raw parameter lists can contain hundreds or thousands of entries;
- names can be blank or repetitive;
- normalized values do not communicate physical units;
- display strings may lag normalized state or be unavailable;
- dynamic controls such as Pro-Q bands must sometimes be activated before other parameters make sense;
- an agent can burn many calls probing a mapping that should be learned once.

This is not an FL Studio problem. FL is faithfully exposing a plugin control surface. The semantic interpretation belongs above the adapter.

## Profile concept

A profile might be keyed by enough identity to detect drift:

```text
PluginProfileKey
  plugin name
  vendor where available
  plugin format
  plugin version/build where observable
  parameter manifest fingerprint
  DAW/adapter version if needed
  profile schema version
```

And contain:

```text
PluginSemanticProfile
  parameter groups
  canonical semantic names
  raw parameter identifiers
  units/display domains
  normalized ↔ semantic mapping where known
  discrete values
  booleans/toggles
  activation dependencies
  settling/verification behavior
  confidence/provenance
```

Example conceptually:

```text
Pro-Q 4 / Band 1
  active        -> Band 1 Used
  enabled       -> Band 1 Enabled
  frequency_hz  -> Band 1 Frequency
  gain_db       -> Band 1 Gain
  q             -> Band 1 Q
  shape         -> Band 1 Shape
```

The profile should describe control semantics, not contain mix recipes such as "cut 350 Hz on vocals."

## Learning profiles

Several sources can contribute:

1. deterministic inspection of published parameter names;
2. known/manual profiles for important plugins;
3. bounded calibration experiments on a disposable instance;
4. user corrections;
5. vendor documentation where legally/practically usable;
6. model-assisted grouping/annotation, accepted only with verification/confidence.

Calibration should happen outside normal production turns where possible. Repeated live probing is expensive and can alter plugin state.

## Profile confidence

Not every mapping is equally trustworthy.

Potential levels:

```text
exact
  manually/vendor verified

observed
  calibrated and read back successfully

inferred
  derived from names/structure but not fully calibrated

unknown
  raw parameter only
```

The tool layer can expose richer semantic controls only at appropriate confidence.

## Agent-facing use

Instead of exposing 1,600 raw parameters to the model, a selected plugin could provide a compact schema:

```text
set_eq_band(
  band = 1,
  enabled = true,
  frequency_hz = 350,
  gain_db = -2.5,
  q = 1.1
)
```

The app resolves that through the plugin profile into raw normalized writes.

Crucially, the raw parameter tools remain available as an escape hatch in expert/developer modes.

## Relation to task-specific apps

Profiles become especially useful when a task projection knows which plugin instance is selected. A general FL agent should not load all profiles into context.

Flow:

```text
user selects plugin
      ↓
app resolves plugin identity
      ↓
load matching semantic profile
      ↓
expose compact semantic tools/context
```

## Versioning and drift

A profile must fail closed when the live parameter manifest no longer matches its fingerprint closely enough.

Do not silently apply an old mapping to a changed plugin version.

The fallback is raw inspection, not guessing.

## Broader multi-DAW value

Plugin semantic profiles are potentially more portable than DAW control adapters because VST3/CLAP/AU plugin semantics travel between hosts even when DAW object models differ.

However, host-exposed parameter order/names can still differ, so profiles may need host-specific fingerprints/adapters rather than assuming perfect portability.

## Risks

- building/maintaining profiles can become a large compatibility burden;
- some plugins expose poor or unstable automation surfaces;
- calibration can be unsafe if run on a user's configured live instance;
- semantic wrappers may hide creative controls advanced users want.

## Score

- FL leverage: **4/5**
- Unlocks: **4/5**
- Differentiation: **4/5**
- Learning: **5/5**
- Effort: **4/5**
- Uncertainty: **4/5**
- Priority score: **19/30**

## Recommendation

**Do not build a large plugin database yet.** Start with one difficult/high-value plugin such as Pro-Q 4 and one compressor, driven by real benchmark failures. Prove that a cached semantic profile reduces calls and improves action quality before expanding coverage.