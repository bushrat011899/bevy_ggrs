use crate::{GgrsSnapshots, LoadWorld, LoadWorldSet, RollbackFrameCount, SaveWorld, SaveWorldSet};
use bevy::prelude::*;
use std::marker::PhantomData;

/// A [`Plugin`] which manages snapshots for a [`Resource`] `R` using [`Copy`].
pub struct GgrsResourceSnapshotCopyPlugin<R>
where
    R: Resource + Copy,
{
    _phantom: PhantomData<R>,
}

type Snapshots<R> = GgrsSnapshots<R, Option<R>>;

impl<R> Default for GgrsResourceSnapshotCopyPlugin<R>
where
    R: Resource + Copy,
{
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<R> GgrsResourceSnapshotCopyPlugin<R>
where
    R: Resource + Copy,
{
    pub fn save(
        mut snapshots: ResMut<Snapshots<R>>,
        frame: Res<RollbackFrameCount>,
        resource: Option<Res<R>>,
    ) {
        snapshots.push(frame.0, resource.map(|res| *res));
    }

    pub fn load(
        mut commands: Commands,
        mut snapshots: ResMut<Snapshots<R>>,
        frame: Res<RollbackFrameCount>,
        resource: Option<ResMut<R>>,
    ) {
        let snapshot = snapshots.rollback(frame.0).get();

        match (resource, snapshot) {
            (Some(mut resource), Some(snapshot)) => *resource = *snapshot,
            (Some(_), None) => commands.remove_resource::<R>(),
            (None, Some(snapshot)) => commands.insert_resource(*snapshot),
            (None, None) => {}
        }
    }
}

impl<R> Plugin for GgrsResourceSnapshotCopyPlugin<R>
where
    R: Resource + Copy,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<Snapshots<R>>()
            .add_systems(
                SaveWorld,
                (Snapshots::<R>::discard_old_snapshots, Self::save)
                    .chain()
                    .in_set(SaveWorldSet::Snapshot),
            )
            .add_systems(LoadWorld, Self::load.in_set(LoadWorldSet::Data));
    }
}
