//! Which meter each of the HUD's nodes belongs to, the fork's own (docs/fork.md): the
//! Glance's **HUD on the Pip-Boy** row takes the four meters the device's own pages show
//! off the screen (`crate::wrist_hud`), and it needs to know which nodes they are.
//!
//! It lives here rather than in `hud.rs` so a sync with upstream NVForever has only the
//! tags themselves to merge in that file: `hud.rs` re-exports these two and marks its
//! nodes with them, nothing more.

use bevy::prelude::*;

/// One of the four meters the Glance's **HUD on the Pip-Boy** row takes off the screen
/// and leaves to the device's own pages (`crate::wrist_hud`): the health bar and its
/// label, the action points bar and its label, the weapon's condition row, the ammo
/// counter. The brackets, the compass, the crosshair and the messages are not among them:
/// only the meters the Pip-Boy's own STATUS page shows [design: the owner, 2026-11-21].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeterKind {
    Hp,
    Ap,
    Cnd,
    Ammo,
}

impl MeterKind {
    /// The four, in the order `crate::wrist_hud` keeps their flashes in.
    pub const ALL: [MeterKind; 4] = [MeterKind::Hp, MeterKind::Ap, MeterKind::Cnd, MeterKind::Ammo];

    pub fn at(self) -> usize {
        self as usize
    }
}

/// The root of a meter's own nodes, so one row can be hidden without the piece it stands
/// in (the compass shares the health piece, the brackets frame both).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Meter(pub MeterKind);

#[cfg(test)]
mod tests {
    use super::*;

    /// Each meter's place is its place in [`MeterKind::ALL`]: what `wrist_hud` keeps its
    /// flashes in, so a kind added to the list cannot land on another's flash.
    #[test]
    fn every_meter_is_in_the_list_once_at_its_own_place() {
        for (i, k) in MeterKind::ALL.iter().enumerate() {
            assert_eq!(k.at(), i, "{k:?}");
        }
    }
}
