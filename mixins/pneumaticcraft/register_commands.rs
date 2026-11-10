// Declare `/pneumaticcraft info|air|energy` (API 8).
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::pneumaticcraft_state::on_register_commands(commands);
}
