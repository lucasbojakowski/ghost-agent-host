# Parameter-space export plan

1. Attach to the live Gopher catalog and require all read/write primitives used by the exporter.
2. Stop transport and verify mixer Insert 47, visual slot 10, is empty.
3. Resolve every requested plugin by exact name through live Browser enumeration or an explicit local `.fst` fallback.
4. For each plugin, add a fresh temporary instance, verify the loaded name, enumerate parameters, read defaults, sample normalized anchors 0/0.25/0.5/0.75/1, restore every default, and remove the instance.
5. Write JSON plus natural-language artifacts per plugin.
6. Validate unique indices, readbacks/restoration, plugin-folder completeness, and final slot cleanup.

Requested plugins: Pro-Q 4
