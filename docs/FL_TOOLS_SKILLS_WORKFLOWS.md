\# Towards a Collection of Domain-Driven Tool Suite for Music Workflows



We are exploring the possibilities of app-driven agent tool composition inside a music production environment, for this project we have access to raw Fruity Loops MIDI Scripting + Gopher MCPTools.

This open up possibilities right of the gate, these offer interfaces into: 



* Plugins 
* Parameters 
* Channels 
* Playlist
* Device
* Arrangement
* Mixer
* Transport
* Piano Roll
* Project Context
* Patterns
* UI 
* and much more



This works, the agent is able to get the catalog of tools and call them with a high degree of precision for diverse tasks; but token efficiency is still an issue.

The agent uses a lot of tokens and calls a lot of tools. We MUST create a comprehensive and domain-driven semantic layer of tools bundled into focused skills.

There are experiments in this project into tool querying so the agent is not fed the whole catalog at first (we did it for the MIDI Scripting catalog which is pretty big), but this is also inneficient. The agent produces a lot of queries even for objective flows which could be handled with a specific skill.



Much of the actions usually consider a get\_some\_context -> set\_something -> get\_some\_context -> ...; we are starting to bypass it by providing a live projection of the project's state, but we need to deepen that into an efficient system. But as one can imagine, chaining multiple of these can be really costly. Consider a task as: "Following the created plan, setup the project channels, playlist tracks, playlist markers, mixer tracks and routing. Name channels and use colors to represent groups.". This starts a cascade of tool calls. We should be able to model actions into tools that allow an agent to fill in a json and output deterministic functions which chain these primitives into complex results, the tool should handle the verification and validation, the agent should get a deterministic response indicating success or error. The degree of chaining is really a matter of choice, agents are really good at filling up JSON. We should also look into programmatic tools, and how to leverage agent's coding capabilities so maybe we nudge an agent to, instead of calling a bunch of tools, it creates a rust, pwsh or any other script which executes the tools; this allows the agent to create conditions, check results programmatically, chain multiple calls, filter out data, and return a compact summary with key data, without exploding context.



This is the general concept we are dealing with, next i am going to specify different Tool categories.



In this document i might refer to workflows as an interchangeable word for tools, skills or the combination of them, in this context the goal is to really expose only skills which bundle knowledge and the tools used for that domain, in a semantic fashion.



\## Channels \& Patterns



Channels represent the midi domain of FL Studio, here we can use the step sequencer, create piano rolls, control panning, volume and other parameters and MIDI CCs. Elements inside a channel can be Generators, Audios, Automations. Generator and Audio might be linked to a mixer insert for further processing.

Patterns serve as a container for channels. Inside a pattern we can organize instruments, one might actually produce an entire song inside a pattern and render the full track without even touching the playlist, of course, this is not ideal and not encouraged at all, but serves to show the power of patterns. Usually a pattern will be named and contain at least one channel's steps or piano roll composition, it might group many channels if necessary.



Gopher and MIDI Scripting offer a reasonable surface of control and visualization into these objects; workflows shall be designed to facilitate manipulation and ergonomics of working with them.



\## Piano Roll



This one is a beast. The name might make us feel like it only handles melody or harmony, but in reality, it encapsulates a lot of actions. We can create midi, which includes notes, velocity, automations, parameters, CCs, ...

Much of our FL Studio actions are gonna include piano roll.



\## Transport



Here we control play, pause, stop, pattern/playlist(song) mode, record, BPM.



\## Project Context



For us it means a structured projection of live (even event-based subscription-style signals) fl studio data/state. It has proven to be a marvelous solution into avoiding huge get\_project\_context calls everytime the agent needs to execute or observe something. We must expand this and provide multi-angled views on diverse sets of fl studio's state.



\## UI



Provides a lot of possibilities into deep actions which are not covered by fl's script + mcp tooling. An deeper exploration into it is necessary.



\## Device



I don't really know what is up with this, we should look into it.



\## Plugins



Plugins are any stock or third party Generator or Effect available inside FL Studio. They come with a parameter topology which can be arbitrarily complex; this creates for us a problem: how to provide precise context about them to an agent so it can traverse these topologies without running long query -> reason -> act -> reason -> query -> ... chains which cause latency, budget degradation and margin for error.

Stock plugins, based on a prior analysis of the fl sample set above, have a direct topology of parameters, which make them a sweet starting point for general processing.

Third party plugins, on the other hand, offer a huge list which sometimes contain thousands of candidates. Just a handful of them really matter to us, so mapping them before asking an agent to start this sad journey is mandatory.



\## Parameters



Parameters are the operational workers in our universe. They allow us to effectively act on objects inside FL Studio to achieve a wide horizon of desired results. As described above, accessing can be dangerous.

A more complete mapping of classes of parameters is an area of exploration we should begin.

They are usually contained inside Channels, Plugins or objects inside fl studio.



\## Playlist



The Playlist offer to us a lot of tools to actually organize our musical elements, in it goes: Patterns, Audio, Automations, and even Generators and Mixer Inserts.

It works alongside Arrangement allowing a component-like architecture to build our musical intent.



\## Arrangement



Exists inside a playlist, it wraps useful actions and projections into the existing playlist content, a deeper mapping of it shall be realized.



\## Mixer



For the mixer, we are dealing with multiple inserts + multiple parameter/insert + multiple slot/insert + multiple parameters/slot. We can also add the Ghost Tap to this, which allows an agent to capture the audio of a specific insert, even adjusting the slot to capture a desired signal step (one could add/move the tap to the first slot to capture the raw signal, or move it after the EQ \& Compressor to capture the result, etc). As we can see things can get complex when dealing with different effects, multiple inserts...



A solution would be to define an effect-space. The user, or system, chooses a set of plugins, example:

(FL-First)



Fruity Parametric EQ 2;

Fruity Reeverb 2;

Fruity Delay 3;

Fruity Compressor;

Fruity Multiband Compressor;

Fruity Soft Clipper;

Soundgoodizer;

Fruity Filter;

Fruity Chorus;

Fruity Phaser;

Fruity Flanger;

Spreader;

Patcher.



(Fabfilter)

Pro-Q 4;

Pro-C 3;

Pro-R 2;

Pro-MB;

Pro-L 2;

Saturn 2;

Pro-DS.



(Izotope)

Neutron Compressor

Neutron Equalizer

Ozone Limiter

Ozone Exciter

...



This can get as long as one may need, our goal is to allow the system to come with a set of pre-validated stock plugin map and allow one to extend with other effects (this also applies to generators, so we can generalize into Plugins)

The system pre-analyses the parameter surface and generates skills/tools guiding the agent into how to properly use the plugin.

(I actually implemented a analysis script which does some clever automation, it instantiates a plugin into a slot, and loops over its parameters testing anchor values and generating artifacts containing the results, we have all FL-First plugins mapped.)



A skill template could be:



skills/

fl-effects-basic/

SKILL.md

eq.md

reverb.md

...



SKILL.md:

```

\---

name: fl-effects-basic

description: Use Fruity Loops plugins such as equalizer, reverb, delay, compressor, multiband compressor, clipper, filter, chorus, flanger, phaser, spreader, patcher

\---



a skill on how to use the plugins with progressive disclosure into the specific plugin file

```



{plugin}.md

```

guide over the type of processing, parameters, tools to get/set values, etc...

```



Current Plugin Probing Module



```

cargo run -q -p fl-gopher-probe -- export-parameter-spaces `

>>   --output "D:\\konko\\ghost\\ghost-agent-host\\artifacts\\plugin-analysis\\{plugin}" `

>>   --probe-track 47 `

>>   --probe-slot 10 `

>>   --plugin-database "C:\\Users\\music\\OneDrive\\Documentos\\Image-Line\\FL Studio\\Presets\\Plugin database\\{Effects, Generators}" `

>>   --allow-local-plugin-database-fallback `

>>   --skip-live-browser-enumeration `

>>   --context-fl-version "Producer Edition v26.1.3 \[build 5570]" `

>>   --plugin "{exact name from the .fst, example: Fruity Reeverb 2"

```



this outputs a bundle of:



a folder with: parameter-space.json with a full analysis over each parameter probed with fl-normalized values + the resulting displayed values, raw-parameter-list.txt, README.md

some additional metadata artifacts.



\## Our Rust System



We are developing a complex rust based application, we could even argue it is actually a collection of application to some degree, in essence we are trying to combine fl studio interfaces with custom modules for many different intentions. These include, but are not limited to:



Audio processing for features such as spectrograms, numerical representations, calculated derivatives, complex deterministic evaluations, etc...

FL Studio interfaces such as the native mcp tooling or the scripting layer which powers our whole project.

HTTP and std interfaces to integrate everything and allow everything to be usable by humans or digital agents. 



We are a long way from where we began. From the initial CLAP meta-plugin which hosted other plugins into the current core crates + apps that are showing us the amount of possibilities we can explore.

The ultimate goal is to develop a robust and evidence-based core SDK which provide primitives and actual workflows that apps must have access to; each crate is usually self-contained and doesn't interact with other crates, this is very important for the architecture, allowing apps to freely choose what goes in and how to use them in their own environment. Usually a crate spawns from an objective necessity or from modules repeated across many apps.

Apps are the implementation of crates + app-local needs; an app is free to work with the primitives and extend them with custom code or other types of content based on the app landscape. Things that are reused across apps, might eventually become part of the SDK crates, but that isn't something that happens immediately by design. 



\## The Svelte Layer



We have plans to develop a Tauri desktop application. For this we begin by developing a Sveltekit baseline, without Tauri for now. Here we are developing the Runtime, a control plane that allows us to check an instance of fl studio or an app vitals, control initialization, etc...

Right now we have an interesting thing going on: we have an apps route which renders an iframe of existing apps made in pure html served by a http server, right now we are using rust for it, with full functionality.

The ultimate goal is to use the runtime sveltekit as the host for apps built with a custom Svelte library of components, allowing apps to be developed on the fly and easily integrated into our ecosystem.



\## Domain Knowledge



Alongside our workflows we must document actual theory, practical insights, articles, professional experiences and any available data we can get online or through llms trained knowledge to build a diverse set of documentation about music, production, mixing, mastering, composition, songwriting, arrangement, aesthetic, audio, synthesis, etc... wrapping genres, styles, periods, ...



This knowledge shall be used to enrich agent's skills, and even creating specific skills based on a lot of things including, but not limited to: specific processing styles or chains, genre guidelines, famous engineers/producers workflows, music fundamentals, templates, etc...



\## Synthesis



This mindset allows us to gather necessary data to effectively design the aggregate rust modules, tooling and skill layers that should power our agents. A deep reflection over our available modules shall result in deterministic and efficient workflows that capture producer intent + agent-driven designs.



