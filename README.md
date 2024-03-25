# 🥚 Moonshine Spawn

A lightweight spawn utility for [Bevy](https://bevyengine.org/).

## Overview

In Bevy, complex hierarchies of entities are typically spawned using [`ChildBuilder`](https://docs.rs/bevy/latest/bevy/prelude/struct.ChildBuilder.html) pattern:

```rust
fn spawn_chicken(commands: &mut Commands) -> Entity {
    // Spawn logic is spread between this function and the bundle
    commands.spawn(ChickenBundle::new()).with_children(|chicken| {
        chicken.spawn(ChickenHead).with_children(|head| {
            head.spawn(ChickenBody).with_children(|body| {
                body.spawn(ChickenLegs)
            });
        });
    })
    .id()
}

#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    // ...
}
```

While this pattern works for most cases, it tends to spread out the logic of entity spawning between the bundle and the function which builds the entity hierarchy which arguably makes the code harder to read and maintain.

There is also no mechanism to statically define an entity hierarchy.

This crate aims to solve both of this issues by introducing statically defined spawnables and allowing bundles to spawn children:

```rust
use moonshine_spawn::prelude::*;

fn init(app: &mut App) {
    // Make sure `SpawnPlugin` is added to your `App`
    app.add_plugins(SpawnPlugin);
}

fn spawn_chicken(commands: &mut Commands) -> Entity {
    // Spawn logic is hidden from this function
    commands.spawn(ChickenBundle::new()).id()
}

#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    // ...
    children: SpawnChildren,
}

impl ChickenBundle {
    // Spawn logic is encapsulated with the bundle
    fn new() -> Self {
        Self {
            chicken: Chicken,
            children: spawn_children(|chicken| {
                chicken.spawn(ChickenHead.with_children(|head| {
                    head.spawn(ChickenBody.with_children(|body| {
                        body.spawn(ChickenLegs)
                    }));
                }));
            })
        }
    }
}


fn register_chicken(app: &mut App) {
    // Register `ChickenBundle` as a spawnable with the key "Chicken"
    // Each spawnable must have a unique key!
    app.add_spawnable("Chicken", ChickenBundle::new());
}

fn spawn_chicken_with_key(commands: &mut Commands) -> Entity {
    // The same key may be used to spawn a spawnable at runtime
    commands.spawn_with_key("Chicken").id()
}
```

### Features
- Bundles with children
- Statically defined spawnables with serializable spawn keys
- Custom spawners
- Lightweight implementation with minimal boilerplate

## Usage

### Spawnables

This crate introduces two new traits: `Spawn` and `SpawnOnce`:

```rust
pub trait Spawn {
    type Output;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output;
}
```

```rust
pub trait SpawnOnce {
    type Output;

    fn spawn_once(self, world: &World, entity: Entity) -> Self::Output;
}
```

`SpawnOnce` is implemented by default for all Bevy bundles. `Spawn` is implemented for all types which implement `SpawnOnce` and can be cloned. This means all bundles which allow cloning also implement `Spawn`.

Any type which implements either of these traits is a "spawnable".

The output of a spawn is always a bundle, which is then inserted into the given `entity` at the end of spawn process.

You may use these traits to define functional spawnables:
```rust
struct Egg;

impl Spawn for Egg {
    type Output = ChickenBundle;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output {
        // TODO: Randomize the chicken's size based on world state
        ChickenBundle::new()
    }
}

fn spawn_from_egg(egg: Egg) -> Entity {
    commands.spawn_with(egg).id()
}
```

### Bundles + Children

To spawn bundles with children, use the `WithChildren` trait:

```rust
#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    // ...
}

fn chicken() -> impl Bundle {
    ChickenBundle {
        chicken: Chicken,
    }
    .with_children(|chicken| {
        // TODO
    })
}
```

Or use the `SpawnChildren` component and the `spawn_children` function:

```rust
#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    // ...
    children: SpawnChildren,
}

fn chicken() -> impl Bundle {
    ChickenBundle {
        chicken: Chicken,
        children: spawn_children(|chicken| {
            // TODO
        })
    }
}
```

### Keys

Spawn keys are a way to reference a spawnable by a unique string.

These keys must be unique within the scope of a `World` and are registered using the `AddSpawnable` extension trait.

Use this to register your spawnables during app initialization:

```rust
app.add_spawnable("Chicken", chicken());
```

You can then spawn a spawnable using a spawn key at runtime, either using `Commands`, `&mut World`:

```rust
let chicken: EntityCommands = commands.spawn_with_key("Chicken");
```

You may also use spawn keys when spawning children of a bundle:

```rust
fn chicken() -> impl Bundle {
    ChickenBundle {
        chicken: Chicken,
        children: spawn_children(|chicken| {
            chicken.spawn_with_key("Chicken/Head");
        })
    }
}
```