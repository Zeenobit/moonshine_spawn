# 🥚 Moonshine Spawn

[![crates.io](https://img.shields.io/crates/v/moonshine-spawn)](https://crates.io/crates/moonshine-spawn)
[![downloads](https://img.shields.io/crates/dr/moonshine-spawn?label=downloads)](https://crates.io/crates/moonshine-spawn)
[![docs.rs](https://docs.rs/moonshine-spawn/badge.svg)](https://docs.rs/moonshine-spawn)
[![license](https://img.shields.io/crates/l/moonshine-spawn)](https://github.com/Zeenobit/moonshine_spawn/blob/main/LICENSE)
[![stars](https://img.shields.io/github/stars/Zeenobit/moonshine_spawn)](https://github.com/Zeenobit/moonshine_spawn)

Collection of tools for spawning entities in [Bevy](https://bevyengine.org/).

## Overview

In Bevy, complex hierarchies of entities are typically spawned using the [`ChildBuilder`](https://docs.rs/bevy/latest/bevy/prelude/struct.ChildBuilder.html):

```rust
use bevy::prelude::*;

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

While this pattern works for most cases, it tends to spread out the logic of entity spawning between the bundle and the function which builds the entity hierarchy. Arguably, this makes the code less modular and harder to read and maintain.

Additionally, there is no built-in functionality to reference a predefined entity hierarchy (i.e. a "prefab").

This crate aims to solve some of these issues by providing tools to make spawning more ergonomic:

```rust
use bevy::prelude::*;
use moonshine_spawn::prelude::*;

let mut app = App::new();
// Make sure `SpawnPlugin` is added to your `App`:
app.add_plugins((DefaultPlugins, SpawnPlugin));

// Register spawnables during initialization:
let chicken_key: SpawnKey = app.add_spawnable("chicken", chicken("Some Chicken"));

// Spawn a spawnable with a key:
let chicken = app.world.spawn_with_key(chicken_key); // .spawn_with_key("chicken") also works!

// Caller does not need to know about `ChickenBundle`:
fn chicken(name: impl Into<Name>) -> impl Spawn {
    ChickenBundle::new()
}

#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    name: Name,
    children: SpawnChildren,
}

impl ChickenBundle {
    // Let's make a chicken!
    fn new(name: impl Into<Name>) -> Self {
        Self {
            chicken: Chicken,
            name: name.into(),
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
```

## Usage

### Spawnables

A type is a "spawnable" if it implements either [`Spawn`] or [`SpawnOnce`]:

```rust,ignore
trait Spawn {
    type Output;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output;
}

trait SpawnOnce {
    type Output;

    fn spawn_once(self, world: &World, entity: Entity) -> Self::Output;
}
```

`SpawnOnce` is implemented by default for all Bevy bundles.

`Spawn` is implemented for all types which implement `SpawnOnce + Clone`. This means any `Bundle + Clone` implements `Spawn`.

The output of a spawn is always a bundle which is then inserted into the given `entity` at the end of spawn process.

You may use these traits to define functional spawnables:
```rust
use bevy::prelude::*;
use moonshine_spawn::prelude::*;

#[derive(Resource)]
struct DefaultChickenName(Name);

struct Egg;

impl Spawn for Egg {
    type Output = ChickenBundle;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output {
        let DefaultChickenName(name) = world.resource::<DefaultChickenName>();
        ChickenBundle::new(name.clone())
    }
}

#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    name: Name,
}

impl ChickenBundle {
    fn new(name: Name) -> Self {
        Self {
            chicken: Chicken,
            name,
        }
    }

}

#[derive(Component)]
struct Chicken;

fn spawn_from_egg(egg: Egg) -> Entity {
    commands.spawn_with(egg).id()
}
```

### Bundles + Children

To spawn bundles with children, use the `WithChildren` trait:

```rust
use bevy::prelude::*;
use moonshine_spawn::prelude::*;

#[derive(Component)]
struct Chicken;

fn chicken() -> impl Bundle {
    Chicken.with_children(|chicken| {
        // ...
    })
}
```

Or use the `SpawnChildren` component and the `spawn_children` function:

```rust
#[derive(Bundle)]
struct ChickenBundle {
    chicken: Chicken,
    children: SpawnChildren,
}

fn chicken() -> impl Bundle {
    ChickenBundle {
        chicken: Chicken,
        children: spawn_children(|chicken| {
            // ...
        })
    }
}
```

### Spawn Keys

A [`SpawnKey`] is a reference to a registered spawnable.

Each key must be unique within the scope of a [`World`] and is registered using the [`AddSpawnable`] extension trait.

Use this to register your spawnables during app initialization:

```rust,ignore
app.add_spawnable("chicken", chicken());
```

You can then spawn a spawnable using a spawn key at runtime, either using `Commands` or a `&mut World`:

```rust,ignore
commands.spawn_with_key("chicken");
```

You may also use spawn keys when spawning children of a bundle:

```rust
fn chicken() -> impl Bundle {
    ChickenBundle {
        chicken: Chicken,
        children: spawn_children(|chicken| {
            chicken.spawn_with_key("chicken_head");
        })
    }
}
```

[`World`]:(https://docs.rs/bevy/latest/bevy/ecs/world/struct.World.html)
[`Spawn`]:(https://docs.rs/moonshine-spawn/latest/moonshine_spawn/trait.Spawn.html)
[`SpawnOnce`]:(https://docs.rs/moonshine-spawn/latest/moonshine_spawn/trait.SpawnOnce.html)
[`SpawnKey`]:(https://docs.rs/moonshine-spawn/latest/moonshine_spawn/struct.SpawnKey.html)
[`AddSpawnable`]:(https://docs.rs/moonshine-spawn/latest/moonshine_spawn/trait.AddSpawnable.html)