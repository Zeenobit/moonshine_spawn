use std::fmt;
use std::hash::{Hash, Hasher};

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::system::EntityCommands;
use bevy_hierarchy::BuildWorldChildren;
use bevy_utils::HashMap;

pub mod prelude {
    pub use super::{
        spawn_children, spawn_key, RegisterSpawnable, Spawn, SpawnChildBuilder, SpawnChildren,
        SpawnCommands, SpawnKey, SpawnOnce, SpawnPlugin, SpawnWorld, Spawnables, WithChildren,
    };
}

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Spawnables::default())
            .add_systems(First, invoke_spawn_children.run_if(should_spawn_children));
    }
}

/// Represents a type which spawns an [`Entity`] exactly once.
///
/// # Usage
/// The output of a spawn is a [`Bundle`] which is inserted into the given spawned [`Entity`].
///
/// By default, all bundles implement this trait.
pub trait SpawnOnce {
    type Output: Bundle;

    #[must_use]
    fn spawn_once(self, world: &World, entity: Entity) -> Self::Output;
}

impl<T: Bundle> SpawnOnce for T {
    type Output = Self;

    fn spawn_once(self, _world: &World, _entity: Entity) -> Self::Output {
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
pub trait Spawn {
    type Output: Bundle;

    #[must_use]
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
pub trait RegisterSpawnable {
    fn register_spawnable<T: Spawn>(self, key: impl Into<SpawnKey>, spawnable: T) -> SpawnKey
    where
        T: 'static + Send + Sync;
}

impl RegisterSpawnable for &mut App {
    fn register_spawnable<T: Spawn>(self, key: impl Into<SpawnKey>, spawnable: T) -> SpawnKey
    where
        T: 'static + Send + Sync,
    {
        self.world
            .resource_mut::<Spawnables>()
            .register(key, spawnable)
    }
}

/// Trait used to spawn spawnables either directly or via a [`SpawnKey`] using [`Commands`].
pub trait SpawnCommands<'w, 's> {
    fn spawn_with<T: SpawnOnce>(&mut self, spawnable: T) -> EntityCommands<'w, 's, '_>
    where
        T: 'static + Send + Sync;

    fn spawn_with_key(&mut self, key: SpawnKey) -> EntityCommands<'w, 's, '_>;
}

impl<'w, 's> SpawnCommands<'w, 's> for Commands<'w, 's> {
    fn spawn_with<T: SpawnOnce>(&mut self, spawnable: T) -> EntityCommands<'w, 's, '_>
    where
        T: 'static + Send + Sync,
    {
        let entity = self.spawn_empty().id();
        self.add(move |world: &mut World| {
            let bundle = spawnable.spawn_once(world, entity);
            world.entity_mut(entity).insert(bundle);
        });
        self.entity(entity)
    }

    fn spawn_with_key(&mut self, key: SpawnKey) -> EntityCommands<'w, 's, '_> {
        let entity = self.spawn_empty().id();
        self.add(move |world: &mut World| {
            if let Some(spawnable) = world.resource_mut::<Spawnables>().take(&key) {
                spawnable.spawn(world, entity);
                world.resource_mut::<Spawnables>().insert(key, spawnable);
            } else {
                panic!("invalid spawn key: {key:?}");
            }
        });
        self.entity(entity)
    }
}

/// Trait used to spawn spawnables either directly or via a [`SpawnKey`] using [`World`].
pub trait SpawnWorld {
    fn spawn_with<T: SpawnOnce>(&mut self, spawnable: T) -> EntityWorldMut
    where
        T: 'static + Send + Sync;

    fn spawn_with_key(&mut self, key: SpawnKey) -> EntityWorldMut;
}

impl SpawnWorld for World {
    fn spawn_with<T: SpawnOnce>(&mut self, spawnable: T) -> EntityWorldMut
    where
        T: 'static + Send + Sync,
    {
        let entity = self.spawn_empty().id();
        let bundle = spawnable.spawn_once(self, entity);
        self.entity_mut(entity).insert(bundle);
        invoke_spawn_children(self);
        self.entity_mut(entity)
    }

    fn spawn_with_key(&mut self, key: SpawnKey) -> EntityWorldMut {
        let entity = self.spawn_empty().id();
        if let Some(spawnable) = self.resource_mut::<Spawnables>().take(&key) {
            spawnable.spawn(self, entity);
            self.resource_mut::<Spawnables>().insert(key, spawnable);
            invoke_spawn_children(self);
        } else {
            panic!("invalid spawn key: {key:?}");
        }
        self.entity_mut(entity)
    }
}

/// A [`Resource`] which contains all registered spawnables.
#[derive(Resource, Default)]
pub struct Spawnables(HashMap<SpawnKey, Box<dyn Spawnable>>);

impl Spawnables {
    /// Registers a spawnable with a unique [`SpawnKey`] and returns it.
    ///
    /// # Warning
    /// This function will panic if the given key is already registered.
    #[must_use]
    pub fn register<T: Spawn>(&mut self, key: impl Into<SpawnKey>, spawnable: T) -> SpawnKey
    where
        T: 'static + Send + Sync,
    {
        let key = key.into();
        let previous = self.0.insert(key.clone(), Box::new(spawnable));
        assert!(previous.is_none(), "spawn key must be unique: {key:?}",);
        key
    }

    /// Returns an iterator over all registered [`SpawnKey`]s.
    pub fn keys(&self) -> impl Iterator<Item = &SpawnKey> {
        self.0.keys()
    }

    #[must_use]
    fn take(&mut self, key: &SpawnKey) -> Option<Box<dyn Spawnable>> {
        self.0.remove(key)
    }

    fn insert(&mut self, key: SpawnKey, spawnable: Box<dyn Spawnable>) {
        self.0.insert(key, spawnable);
    }
}

/// A unique string-based identifier used to spawn a spawnable registered with [`Spawnables`].
#[derive(Clone)]
pub enum SpawnKey {
    Dynamic(String),
    Static(&'static str),
}

impl SpawnKey {
    pub fn name(&self) -> &str {
        match self {
            Self::Dynamic(name) => name,
            Self::Static(name) => name,
        }
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

impl fmt::Debug for SpawnKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpawnKey").field(&self.name()).finish()
    }
}

impl From<String> for SpawnKey {
    fn from(name: String) -> Self {
        Self::Dynamic(name)
    }
}

impl From<&str> for SpawnKey {
    fn from(name: &str) -> Self {
        Self::Dynamic(name.to_string())
    }
}

/// A function for defining a new static [`SpawnKey`].
///
/// # Example
/// ```
/// # use moonshine_spawn::prelude::*;
/// const FOO: SpawnKey = spawn_key("FOO");
/// ```
pub const fn spawn_key(name: &'static str) -> SpawnKey {
    SpawnKey::Static(name)
}

/// A convenient macro for defining static [`SpawnKey`]s.
/// # Example
/// ```
/// # use moonshine_spawn::prelude::*;
/// spawn_key!(FOO);
/// spawn_key!(BAR, BAZ);
/// ```
#[macro_export]
macro_rules! spawn_key {
    ($i:ident) => {
        const $i: $crate::SpawnKey = $crate::spawn_key(stringify!($i));
    };
    ($($i:ident),*) => {
        $(spawn_key!($i);)*
    };
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

/// A [`Component`] which stores a list of spawnables to spawn as children of its entity.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SpawnChildren(Vec<Box<dyn SpawnableOnce>>);

impl SpawnChildren {
    #[must_use]
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add_child<T: SpawnOnce>(&mut self, spawnable: T)
    where
        T: 'static + Send + Sync,
    {
        self.0.push(Box::new(spawnable));
    }

    fn add_child_with_key(&mut self, key: SpawnKey) {
        self.0.push(Box::new(SpawnKeySpawnable(key)));
    }

    fn invoke(world: &mut World, entity: Entity, mut child_spawned: impl FnMut(Entity)) {
        if let Some(children) = world.entity_mut(entity).take::<SpawnChildren>() {
            for spawnable in children.0 {
                let child = world.spawn_empty().id();
                spawnable.spawn_once(world, child);
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
    pub fn spawn<T: SpawnOnce>(&mut self, spawnable: T) -> &mut Self
    where
        T: 'static + Send + Sync,
    {
        self.0.add_child(spawnable);
        self
    }

    pub fn spawn_with_key(&mut self, key: SpawnKey) -> &mut Self {
        self.0.add_child_with_key(key);
        self
    }
}

trait Spawnable: 'static + Send + Sync {
    fn spawn(&self, world: &mut World, entity: Entity);
}

impl<T: Spawn> Spawnable for T
where
    T: 'static + Send + Sync,
{
    fn spawn(&self, world: &mut World, entity: Entity) {
        let bundle = self.spawn(world, entity);
        world.entity_mut(entity).insert(bundle);
    }
}

trait SpawnableOnce: 'static + Send + Sync {
    fn spawn_once(self: Box<Self>, world: &mut World, entity: Entity);
}

impl<T: SpawnOnce> SpawnableOnce for T
where
    T: 'static + Send + Sync,
{
    fn spawn_once(self: Box<Self>, world: &mut World, entity: Entity) {
        let bundle = (*self).spawn_once(world, entity);
        world.entity_mut(entity).insert(bundle);
    }
}

struct SpawnKeySpawnable(SpawnKey);

impl SpawnableOnce for SpawnKeySpawnable {
    fn spawn_once(self: Box<Self>, world: &mut World, entity: Entity) {
        let Self(key) = *self;
        if let Some(spawnable) = world.resource_mut::<Spawnables>().take(&key) {
            spawnable.spawn(world, entity);
            world.resource_mut::<Spawnables>().insert(key, spawnable);
        } else {
            panic!("invalid spawn key: {key:?}");
        }
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

    spawn_key!(FOO, BAR);

    #[test]
    fn spawn_bundle() {
        let mut app = app();
        let world = &mut app.world;
        let entity = world.spawn_with(Foo).id();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_bundle_deferred() {
        let mut app = app();
        let entity = {
            let world = &mut app.world;
            world.run_system_once(|mut commands: Commands| commands.spawn_with(Foo).id())
        };
        app.update();
        let world = app.world;
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_with_key() {
        let mut app = app();
        app.register_spawnable(FOO, Foo);
        let world = &mut app.world;
        let entity = world.spawn_with_key(FOO).id();
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_with_key_deferred() {
        let mut app = app();
        app.register_spawnable(FOO, Foo);
        let entity = {
            let world = &mut app.world;
            world.run_system_once(|mut commands: Commands| commands.spawn_with_key(FOO).id())
        };
        app.update();
        let world = app.world;
        assert!(world.entity(entity).contains::<Foo>());
    }

    #[test]
    fn spawn_bundle_with_children() {
        let mut app = app();
        let world = &mut app.world;
        let entity = world
            .spawn_with(Foo.with_children(|foo| {
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
            let world = &mut app.world;
            world.run_system_once(|mut commands: Commands| {
                commands
                    .spawn_with(Foo.with_children(|foo| {
                        foo.spawn(Bar);
                    }))
                    .id()
            })
        };
        app.update();
        let world = app.world;
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }

    #[test]
    fn spawn_bundle_with_children_with_key() {
        let mut app = app();
        app.register_spawnable(BAR, Bar);
        let world = &mut app.world;
        let entity = world
            .spawn_with(Foo.with_children(|foo| {
                foo.spawn_with_key(BAR);
            }))
            .id();
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }

    #[test]
    fn spawn_bundle_with_children_with_key_deferred() {
        let mut app = app();
        app.register_spawnable(BAR, Bar);
        let entity = {
            let world = &mut app.world;
            world.run_system_once(|mut commands: Commands| {
                commands
                    .spawn_with(Foo.with_children(|foo| {
                        foo.spawn_with_key(BAR);
                    }))
                    .id()
            })
        };
        app.update();
        let world = app.world;
        let children = world.entity(entity).get::<Children>().unwrap();
        let child = children.iter().copied().next().unwrap();
        assert!(world.entity(child).contains::<Bar>());
    }
}
