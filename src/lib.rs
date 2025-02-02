#![deprecated(
    since = "0.2.4",
    note = "see documentation at https://github.com/Zeenobit/moonshine_spawn for details"
)]
#![doc = include_str!("../README.md")]

use std::fmt::{Debug, Formatter, Result as FormatResult};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemConfigs;
use bevy_ecs::system::EntityCommands;
use bevy_hierarchy::BuildChildren;
use bevy_reflect::prelude::*;
use bevy_utils::HashMap;

pub mod prelude {
    pub use super::{
        spawn_children, AddSpawnable, Spawn, SpawnChildBuilder, SpawnChildren, SpawnCommands,
        SpawnKey, SpawnOnce, SpawnPlugin, SpawnWorld, Spawnables, WithChildren,
    };
}

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SpawnKey>()
            .insert_resource(Spawnables::default())
            .add_systems(First, invoke_spawn_children.run_if(should_spawn_children));
    }
}

/// Represents a type which spawns an [`Entity`] exactly once.
///
/// # Usage
/// The output of a spawn is a [`Bundle`] which is inserted into the given spawned [`Entity`].
///
/// By default, all bundles implement this trait.
pub trait SpawnOnce: 'static + Send + Sync {
    type Output: Bundle;

    fn spawn_once(self, world: &World, entity: Entity) -> Self::Output;
}

impl<T: Bundle> SpawnOnce for T {
    type Output = Self;

    fn spawn_once(self, _: &World, _: Entity) -> Self::Output {
        self
    }
}

/// Represents a type which spawns an [`Entity`].
///
/// # Usage
/// The output of a spawn is a [`Bundle`] which is inserted into the given spawned [`Entity`].
///
/// By default, anything which implements [`SpawnOnce`] and [`Clone`] also implements this trait.
/// This includes bundles which can be cloned.
pub trait Spawn: 'static + Send + Sync {
    type Output: Bundle;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output;
}

impl<T: SpawnOnce + Clone> Spawn for T {
    type Output = T::Output;

    fn spawn(&self, world: &World, entity: Entity) -> Self::Output {
        self.clone().spawn_once(world, entity)
    }
}

/// Trait used to register a spawnable with an [`App`].
///
/// # Usage
/// A spawnable is any thing which implements [`Spawn`]. A spawnable is registered by a unique [`SpawnKey`].
/// This spawn key may then be used to spawn a new instance of the spawnable.
pub trait AddSpawnable {
    fn add_spawnable(self, key: impl Into<SpawnKey>, _: impl Spawn) -> SpawnKey;
}

impl AddSpawnable for &mut App {
    fn add_spawnable(self, key: impl Into<SpawnKey>, spawnable: impl Spawn) -> SpawnKey {
        self.world_mut()
            .resource_mut::<Spawnables>()
            .register(key, spawnable)
    }
}

/// Trait used to spawn spawnables either directly or via a [`SpawnKey`] using [`Commands`].
pub trait SpawnCommands {
    fn spawn_with(&mut self, _: impl Spawn) -> EntityCommands<'_>;

    fn spawn_once_with(&mut self, _: impl SpawnOnce) -> EntityCommands<'_>;

    fn spawn_key(&mut self, key: impl Into<SpawnKey>) -> EntityCommands<'_>;

    fn spawn_key_with(
        &mut self,
        key: impl Into<SpawnKey>,
        bundle: impl Bundle,
    ) -> EntityCommands<'_>;
}

impl SpawnCommands for Commands<'_, '_> {
    fn spawn_with(&mut self, spawnable: impl Spawn) -> EntityCommands<'_> {
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            Spawnable::spawn(&spawnable, world, entity);
        });
        self.entity(entity)
    }

    fn spawn_once_with(&mut self, spawnable: impl SpawnOnce) -> EntityCommands<'_> {
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            SpawnableOnce::spawn_once(spawnable, world, entity);
        });
        self.entity(entity)
    }

    fn spawn_key(&mut self, key: impl Into<SpawnKey>) -> EntityCommands<'_> {
        let key: SpawnKey = key.into();
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            key.spawn_once(world, entity);
        });
        self.entity(entity)
    }

    fn spawn_key_with(
        &mut self,
        key: impl Into<SpawnKey>,
        bundle: impl Bundle,
    ) -> EntityCommands<'_> {
        let key = key.into();
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            SpawnKeyWith(key, bundle).spawn_once(world, entity);
        });
        self.entity(entity)
    }
}

/// Trait used to spawn spawnables either directly or via a [`SpawnKey`] using [`World`].
pub trait SpawnWorld {
    fn spawn_with(&mut self, _: impl Spawn) -> EntityWorldMut;

    fn spawn_once_with(&mut self, _: impl SpawnOnce) -> EntityWorldMut;

    fn spawn_key(&mut self, key: impl Into<SpawnKey>) -> EntityWorldMut;

    fn spawn_key_with(&mut self, key: impl Into<SpawnKey>, bundle: impl Bundle) -> EntityWorldMut;
}

impl SpawnWorld for World {
    fn spawn_with(&mut self, spawnable: impl Spawn) -> EntityWorldMut {
        let entity = self.spawn_empty().id();
        Spawnable::spawn(&spawnable, self, entity);
        invoke_spawn_children(self);
        self.entity_mut(entity)
    }

    fn spawn_once_with(&mut self, spawnable: impl SpawnOnce) -> EntityWorldMut {
        let entity = self.spawn_empty().id();
        SpawnableOnce::spawn_once(spawnable, self, entity);
        invoke_spawn_children(self);
        self.entity_mut(entity)
    }

    fn spawn_key(&mut self, key: impl Into<SpawnKey>) -> EntityWorldMut {
        let key: SpawnKey = key.into();
        let entity = self.spawn_empty().id();
        key.spawn_once(self, entity);
        invoke_spawn_children(self);
        self.entity_mut(entity)
    }

    fn spawn_key_with(&mut self, key: impl Into<SpawnKey>, bundle: impl Bundle) -> EntityWorldMut {
        let key = key.into();
        let entity = self.spawn_empty().id();
        SpawnKeyWith(key, bundle).spawn_once(self, entity);
        invoke_spawn_children(self);
        self.entity_mut(entity)
    }
}

/// A [`Resource`] which contains all registered spawnables.
#[derive(Resource, Default)]
pub struct Spawnables(HashMap<SpawnKey, Arc<dyn Spawnable>>);

impl Spawnables {
    /// Registers a spawnable with a unique [`SpawnKey`] and returns it.
    ///
    /// # Warning
    /// This function will panic if the given key is already registered.
    pub fn register<T>(&mut self, key: impl Into<SpawnKey>, spawnable: T) -> SpawnKey
    where
        T: 'static + Spawn + Send + Sync,
    {
        let key = key.into();
        let previous = self.0.insert(key.clone(), Arc::new(spawnable));
        assert!(previous.is_none(), "spawn key must be unique: {key:?}",);
        key
    }

    /// Returns an iterator over all registered [`SpawnKey`]s.
    pub fn keys(&self) -> impl Iterator<Item = &SpawnKey> {
        self.0.keys()
    }

    fn fetch(&self, key: &SpawnKey) -> Option<Arc<dyn Spawnable>> {
        self.0.get(key).cloned()
    }
}

/// A unique string-based identifier used to spawn a spawnable registered with [`Spawnables`].
#[derive(Clone, Reflect)]
pub struct SpawnKey(String);

impl SpawnKey {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl PartialEq for SpawnKey {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for SpawnKey {}

impl Hash for SpawnKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state)
    }
}

impl Debug for SpawnKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> FormatResult {
        f.debug_tuple("SpawnKey").field(&self.name()).finish()
    }
}

impl From<String> for SpawnKey {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&str> for SpawnKey {
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}

/// Trait used to attach children to an [`Entity`] using a [`Bundle`].
///
/// # Example
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_spawn::prelude::*;
///
/// #[derive(Component)]
/// struct Foo;
///
/// #[derive(Component)]
/// struct Bar;
///
/// let mut world = World::default();
/// world.spawn(Foo.with_children(|foo| {
///    foo.spawn(Bar);
/// }));
/// ```
pub trait WithChildren: Bundle + Sized {
    fn with_children(self, f: impl FnOnce(&mut SpawnChildBuilder)) -> (Self, SpawnChildren);
}

impl<T: Bundle> WithChildren for T {
    fn with_children(self, f: impl FnOnce(&mut SpawnChildBuilder)) -> (Self, SpawnChildren) {
        let mut children = SpawnChildren::new();
        let mut builder = SpawnChildBuilder(&mut children);
        f(&mut builder);
        (self, children)
    }
}

/// A [`Component`] which stores a list of spawnables to spawn as children of its [`Entity`].
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SpawnChildren(Vec<Box<dyn SpawnableOnce>>);

impl SpawnChildren {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add_child(&mut self, spawnable: impl SpawnableOnce) {
        self.0.push(Box::new(spawnable));
    }

    fn add_child_with_key(&mut self, key: SpawnKey) {
        self.0.push(Box::new(key));
    }

    fn invoke(world: &mut World, entity: Entity, mut child_spawned: impl FnMut(Entity)) {
        if let Some(children) = world.entity_mut(entity).take::<SpawnChildren>() {
            for spawnable in children.0 {
                let child = world.spawn_empty().id();
                spawnable.spawn_once_dyn(world, child);
                child_spawned(child);
                world.entity_mut(entity).add_child(child);
            }
        }
    }
}

/// An ergonomic function used to create a [`SpawnChildren`] component.
#[must_use]
pub fn spawn_children(f: impl FnOnce(&mut SpawnChildBuilder)) -> SpawnChildren {
    let mut children = SpawnChildren::new();
    f(&mut SpawnChildBuilder(&mut children));
    children
}

impl Default for SpawnChildren {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SpawnChildBuilder<'a>(&'a mut SpawnChildren);

impl SpawnChildBuilder<'_> {
    pub fn spawn(&mut self, spawnable: impl SpawnOnce) -> &mut Self {
        self.0.add_child(spawnable);
        self
    }

    pub fn spawn_key(&mut self, key: impl Into<SpawnKey>) -> &mut Self {
        self.0.add_child_with_key(key.into());
        self
    }

    pub fn spawn_key_with(&mut self, key: impl Into<SpawnKey>, bundle: impl Bundle) -> &mut Self {
        self.0.add_child(SpawnKeyWith(key.into(), bundle));
        self
    }
}

trait Spawnable: 'static + Send + Sync {
    fn spawn(&self, world: &mut World, entity: Entity);
}

impl<T: Spawn> Spawnable for T {
    fn spawn(&self, world: &mut World, entity: Entity) {
        let bundle = self.spawn(world, entity);
        world.entity_mut(entity).insert(bundle);
    }
}

trait SpawnableOnce: 'static + Send + Sync {
    fn spawn_once(self, world: &mut World, entity: Entity);

    fn spawn_once_dyn(self: Box<Self>, world: &mut World, entity: Entity);
}

impl<T: SpawnOnce> SpawnableOnce for T {
    fn spawn_once(self, world: &mut World, entity: Entity) {
        let bundle = self.spawn_once(world, entity);
        world.entity_mut(entity).insert(bundle);
    }

    fn spawn_once_dyn(self: Box<Self>, world: &mut World, entity: Entity) {
        SpawnableOnce::spawn_once(*self, world, entity);
    }
}

impl SpawnableOnce for SpawnKey {
    fn spawn_once(self, world: &mut World, entity: Entity) {
        if let Some(spawnable) = world.resource::<Spawnables>().fetch(&self) {
            spawnable.spawn(world, entity);
        } else {
            panic!("invalid spawn key: {self:?}");
        }
    }

    fn spawn_once_dyn(self: Box<Self>, world: &mut World, entity: Entity) {
        SpawnableOnce::spawn_once(*self, world, entity);
    }
}

struct SpawnKeyWith<T>(SpawnKey, T);

impl<T: Bundle> SpawnableOnce for SpawnKeyWith<T> {
    fn spawn_once(self, world: &mut World, entity: Entity) {
        self.0.spawn_once(world, entity);
        world.entity_mut(entity).insert(self.1);
    }

    fn spawn_once_dyn(self: Box<Self>, world: &mut World, entity: Entity) {
        SpawnableOnce::spawn_once(*self, world, entity);
    }
}

fn should_spawn_children(query: Query<(), With<SpawnChildren>>) -> bool {
    !query.is_empty()
}

fn invoke_spawn_children(world: &mut World) {
    let mut entities = Vec::new();

    for entity in world.iter_entities() {
        if entity.contains::<SpawnChildren>() {
            entities.push(entity.id());
        }
    }

    while !entities.is_empty() {
        let batch = std::mem::take(&mut entities);
        for entity in batch {
            SpawnChildren::invoke(world, entity, |child| entities.push(child));
        }
    }
}

/// Returns a [`SystemConfigs`] which immediately spawns all pending [`SpawnChildren`] requests.
///
/// # Usage
/// Typically, the spawn system spawns children automatically during [`First`] schedule.
/// In some cases, however, it may be necessary to forcibly spawn children due to ordering issues.
///
/// # Example
/// ```
/// use bevy::prelude::*;
/// use moonshine_spawn::{prelude::*, force_spawn_children};
///
/// #[derive(Component)]
/// struct Bar;
///
/// #[derive(Bundle)]
/// struct Foo {
///     children: SpawnChildren,
/// }
///
/// impl Foo {
///     fn new() -> Self {
///         Self {
///             children: spawn_children(|parent| {
///                 parent.spawn(Bar);
///             })
///         }
///     }
/// }
///
/// fn setup(mut commands: Commands) {
///     commands.spawn(Foo::new());
/// }
///
/// fn post_setup(bar: Query<&Bar>) {
///     let _ = bar.single();
///     // ...
/// }
///
/// App::new()
///     .add_plugins((MinimalPlugins, SpawnPlugin))
///     // Without `force_spawn_children()`, `post_setup` would panic!
///     .add_systems(Startup, (setup, force_spawn_children(), post_setup).chain())
///     .update();
/// ```
pub fn force_spawn_children() -> SystemConfigs {
    invoke_spawn_children.run_if(should_spawn_children)
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::system::RunSystemOnce, prelude::*};

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SpawnPlugin));
        app
    }

    #[derive(Component, Clone)]
    struct Foo;

    #[derive(Component, Clone)]
    struct Bar;

    #[test]
    fn spawn_bundle() {
        let mut app = app();
        let world = app.world_mut();
        let entity = world.spawn_once_with(Foo).id();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_bundle_deferred() {
        let mut app = app();
        let entity = {
            let world = app.world_mut();
            world
                .run_system_once(|mut commands: Commands| commands.spawn_once_with(Foo).id())
                .unwrap()
        };
        app.update();
        let world = app.world();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_with_key() {
        let mut app = app();
        app.add_spawnable("FOO", Foo);
        let world = app.world_mut();
        let entity = world.spawn_key("FOO").id();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_with_key_deferred() {
        let mut app = app();
        app.add_spawnable("FOO", Foo);
        let entity = {
            let world = app.world_mut();
            world
                .run_system_once(|mut commands: Commands| commands.spawn_key("FOO").id())
                .unwrap()
        };
        app.update();
        let world = app.world();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_bundle_with_children() {
        let mut app = app();
        let world = app.world_mut();
        let entity = world
            .spawn_once_with(Foo.with_children(|foo| {
                foo.spawn(Bar);
            }))
            .id();
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }

    #[test]
    fn spawn_bundle_with_children_deferred() {
        let mut app = app();
        let entity = {
            let world = app.world_mut();
            world
                .run_system_once(|mut commands: Commands| {
                    commands
                        .spawn_once_with(Foo.with_children(|foo| {
                            foo.spawn(Bar);
                        }))
                        .id()
                })
                .unwrap()
        };
        app.update();
        let world = app.world();
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }

    #[test]
    fn spawn_bundle_with_children_with_key() {
        let mut app = app();
        app.add_spawnable("BAR", Bar);
        let world = app.world_mut();
        let entity = world
            .spawn_once_with(Foo.with_children(|foo| {
                foo.spawn_key("BAR");
            }))
            .id();
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }

    #[test]
    fn spawn_bundle_with_children_with_key_deferred() {
        let mut app = app();
        app.add_spawnable("BAR", Bar);
        let entity = {
            let world = app.world_mut();
            world
                .run_system_once(|mut commands: Commands| {
                    commands
                        .spawn_once_with(Foo.with_children(|foo| {
                            foo.spawn_key("BAR");
                        }))
                        .id()
                })
                .unwrap()
        };
        app.update();
        let world = app.world();
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }
}
