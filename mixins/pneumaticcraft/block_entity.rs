// Both slice-1 machines get a block entity (API 15/129).
fn observe_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::pneumaticcraft_state::on_block_entity(block, entity);
}
