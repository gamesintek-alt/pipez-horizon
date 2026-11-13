# Native chemical-provider bridge

This is an explicit adapter contract for cooperating Rust mods, not the Java Mekanism API.
An external native block entity opts in with this saved NBT compound:

```text
pipez_chemical: {
  id: "provider:oxygen",
  amount: 1000L,
  capacity: 10000L,
  insert: 1L,
  extract: 1L
}
```

Amounts and capacities must be nonnegative. A provider must keep its amount within capacity.
The `id` may be empty in an empty tank; a nonempty tank accepts only the same ID.
This first bridge has one tank per block, the same access on all faces, and no additional chemical components or chemical-tag registry.
Absent `insert`/`extract` flags default to enabled. A zero flag disables that action.
Updates run on the authoritative simulation thread and mark/sync both block entities.
No provider, generator or Mekanism content is added by this package.

Item, fluid and energy transport use Horizon's own container/fluid/energy capabilities instead of this bridge.
