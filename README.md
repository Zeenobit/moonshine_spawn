# 🥚 Moonshine Spawn

A lightweight spawn utility for [Bevy](https://bevyengine.org/).

## Overview

In Bevy, complex hierarchies of entities are typically spawned using [`ChildBuilder`](https://docs.rs/bevy/latest/bevy/prelude/struct.ChildBuilder.html) pattern:

```rust
fn spawn_chicken(commands: &mut Commands) -> Entity {
    // Spawn logic is spread between this function and the bundle
    commands.spawn(ChickenBundle::new()).with_children(|chicken| {
        chicken.spawn(ChickenHead.with_children(|head| {
            head.spawn(ChickenBody.with_children(|body| {
                body.spawn(ChickenLegs)
            }));
        }));
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

// Define a static spawn key for Chickens
spawn_key!(CHICKEN);

fn register_chicken(app: &mut App) {
    // Register `ChickenBundle` as a spawnable
    app.register_spawnable(CHICKEN, ChickenBundle::new());
}

fn spawn_chicken_with_key(commands: &mut Commands) -> Entity {
    commands.spawn_with_key(CHICKEN).id()
}
```

### Features
- Bundles with children
- Statically defined spawnables
- Custom functional spawnables
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

Or use the `SpawnChildren` component:

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

### Static Spawnables

You can define a static spawnable using a `SpawnKey`:
```rust
const CHICKEN: SpawnKey = spawn_key("CHICKEN");
```

Spawn keys must be unique within the scope of a `World` and are registered using the `RegisterSpawnable` extension trait.

Use this to register your spawnables during app initialization:

```rust
app.register_spawnable(CHICKEN, chicken());
```

You may also use the `spawn_key!` macro to conveniently define one or more spawn keys that match their string representation:

```rust
spawn_key!(CHICKEN);

spawn_key!(
    LARGE_CHICKEN,
    SMALL_CHICKEN
);
```