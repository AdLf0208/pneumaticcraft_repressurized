//! PneumaticCraft: Repressurized for Horizon — slice 1, the FE/air conversion pair.
//!
//! One flat SDK package (mod API 0.2): the carried blockstates, models, textures, lang, loot
//! tables and tags are the `assets/` and `data/` here, and the behaviour is the one
//! `impl Hooks` below. The rules themselves live in [`runtime`] (slice 1, byte-identical to the
//! retired `.hnmod` bundle's `runtime.rs`) and [`tubes`] (slice 2, the pressure-tube network);
//! [`support`] is the glue.
//!
//! # Where the retired container's hooks went
//!
//! The 2026-11-17 mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`: the plan, the
//! injection points and the per-hook helper files are gone. A hook is a typed method here, and a
//! moment that belongs to one block entity is a **Part** attached to the type declared for it.
//!
//! | `.hnmod` hook (API) | here |
//! |---|---|
//! | the `content_root` injection (AE2 pattern) | gone: one flat `.hmod` carries `assets/` and `data/` itself |
//! | `block_entity` (15) | the two block entity **types** declared in [`Mod::register`] |
//! | `block_entity_ticks` (15) | `BlockEntityPart::block_entity_ticks` on both Parts |
//! | `block_entity_energy` (142) | `Registrar::block_energy` in [`Mod::register`] |
//! | `block_entity_tick` (17) | `BlockEntityPart::block_entity_tick` on both Parts |
//! | `register_commands` (8) | [`Mod::register_commands`] |
//! | `perform_command` (8) | [`Mod::run_mod_command`] |
pub mod runtime;
pub mod support;
pub mod tubes;

use mod_api::{
    self, Flow, Hooks, HostSide, LevelTickPostEvent, RegisterCommandsEvent, Registrar,
    RegistrationContext, RunModCommandEvent, ServerHost,
};

/// The mod keeps no state of its own: everything a machine knows is in its block entity's saved
/// compound, so there is nothing here to share between calls.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare what the game answers for the two blocks (`EntityBlock` plus NeoForge's
    /// `RegisterCapabilitiesEvent`): a block entity type each, its 100,000 FE buffer, and the
    /// Part that makes it tick.
    fn register(&self, registrar: &mut dyn Registrar, context: &RegistrationContext) {
        support::declare(registrar, context);
    }

    /// `/pneumaticcraft info|air|energy`: the harness controls upstream has none of (precedent
    /// `/powah info|set`, `/charginggadgets energy`).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::declare_commands(event.commands);
    }

    /// The command bodies: the dispatcher has already parsed the position and the amount.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::run_command(host, event)
    }

    /// After a level's block entities have ticked: hand each handler what was queued for it — the
    /// air its neighbours sent, and the harness's own sets (`level_tick_post`, API 1). A write
    /// made inside a block entity's tick is overwritten by that tick's write-back, so everything
    /// the pressure network does to a handler is applied here, once every tick has finished.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        support::apply_pending(host, event.level);
    }
}
