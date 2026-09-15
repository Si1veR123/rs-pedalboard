use crate::{
    pedalboard_set, pedals::{Pedal, PedalParameterValue, PedalTrait}, processor_settings::FloatSettingUpdate, unique_time_id,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, hash::Hash};

/// Can uniquely identify a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParameterPath {
    pub pedalboard_id: Option<u32>,
    pub pedal_id: u32,
    pub parameter_name: String,
}

impl ParameterPath {
    /// Find the pedalboard that contains the pedal with this path's pedal ID and set
    /// `pedalboard_id`.
    ///
    /// The active pedalboard is preferred, which disambiguates the case where the same pedal ID
    /// appears in several pedalboards. Note that pedal IDs are only unique within a pedalboard,
    /// not across pedalboards (for example, duplicated pedalboards keep their pedal IDs).
    ///
    /// Returns whether the path now points at a pedalboard in the set.
    pub fn resolve_pedalboard_id(
        &mut self,
        pedalboard_set: &pedalboard_set::PedalboardSet,
    ) -> bool {
        if self.pedalboard_id.is_none() {
            let pedal_id = self.pedal_id;
            let contains_pedal =
                |pedalboard: &&Pedalboard| pedalboard.pedals.iter().any(|p| p.get_id() == pedal_id);

            // Prefer the active pedalboard so ambiguous pedal IDs resolve to the board in view.
            let active_pedalboard = pedalboard_set
                .pedalboards
                .get(pedalboard_set.active_pedalboard)
                .filter(|pedalboard| contains_pedal(pedalboard));

            let containing_pedalboard = active_pedalboard.or_else(|| {
                pedalboard_set
                    .pedalboards
                    .iter()
                    .find(|pedalboard| contains_pedal(pedalboard))
            });

            self.pedalboard_id = containing_pedalboard.map(|pedalboard| pedalboard.get_id());
        }

        self.pedalboard_id.is_some()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Pedalboard {
    // This ID is not necessarily unique in a PedalboardSet,
    // however Pedalboards with the same ID should be functionally equal
    // (same pedals, same parameters, same name, etc)
    #[serde(default)]
    id: u32,
    #[serde(default)]
    pub name: String,
    pub pedals: Vec<Pedal>,

    #[serde(skip)]
    prepend_message: String,
    #[serde(skip)]
    pedal_message_buffer: Vec<String>,
}

impl std::fmt::Debug for Pedalboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pedalboard {{ id: {}, name: {}, pedals count: {:?} }}",
            self.id,
            self.name,
            self.pedals.len()
        )
    }
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            id: unique_time_id(),
            name: String::from("Default Pedalboard"),
            pedals: vec![Pedal::Volume(crate::pedals::Volume::new())],
            prepend_message: String::new(),
            pedal_message_buffer: Vec::with_capacity(12),
        }
    }
}

impl Hash for &Pedalboard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Pedalboard {
    /// Has a volume pedal by default
    pub fn new(name: String) -> Pedalboard {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    pub fn from_pedals(name: String, pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard {
            id: unique_time_id(),
            name,
            pedals,
            ..Default::default()
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        self.pedals.iter_mut().for_each(|pedal| {
            if !pedal.is_active() {
                return;
            }

            // Clear the message buffer for each pedal
            self.pedal_message_buffer.clear();

            pedal.process_audio(buffer, &mut self.pedal_message_buffer);

            // only build the prefix if the pedal sent a message
            if !self.pedal_message_buffer.is_empty() {
                self.prepend_message.clear();
                if let Err(e) = write!(&mut self.prepend_message, "pedalmsg{} ", pedal.get_id()) {
                    tracing::warn!("Failed to write prepend message: {}", e);
                }

                for message in &mut self.pedal_message_buffer {
                    message.insert_str(0, &self.prepend_message);
                }
            }

            message_buffer.append(&mut self.pedal_message_buffer);
        });
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// The parameter which a device at the given sensible mapping `index` should control.
    /// Returns `None` when the pedalboard has no suitable parameter at that index.
    pub fn sensible_parameter_from_index(
        &self,
        index: usize,
        update: &FloatSettingUpdate,
    ) -> Option<ParameterPath> {
        let (pedal_id, parameter_name) = match update {
            // For FlipFlop update find an 'Active' parameter
            FloatSettingUpdate::FlipFlop => {
                // Find the 'Active' parameter of the pedal at the given index
                let pedal_id = self
                    .pedals
                    .iter()
                    .filter(|pedal| pedal.get_parameters().contains_key("Active"))
                    .map(|pedal| pedal.get_id())
                    .nth(index)?;

                (pedal_id, "Active".to_string())
            }
            // For other updates, find a numerical parameter
            _ => self.sensible_parameters().into_iter().nth(index)?,
        };

        Some(ParameterPath {
            pedalboard_id: Some(self.id),
            pedal_id,
            parameter_name,
        })
    }

    /// Every parameter which a sensible mapping index can refer to, in a fixed order.
    fn sensible_parameters(&self) -> Vec<(u32, String)> {
        // Order of parameters likely to be of 'high importance'
        const PRIORITISE_PARAMETERS: [&str; 5] = ["Gain", "Drive", "Level", "Dry/Wet", "Tone"];

        // Rank (index into PRIORITISE_PARAMETERS), pedal order (index into self.pedals), pedal ID, parameter name
        let mut prioritised: Vec<(usize, usize, u32, String)> = Vec::new();
        let mut fallback_parameters: Vec<(u32, String)> = Vec::new();

        for (pedal_order, pedal) in self.pedals.iter().enumerate() {
            let pedal_id = pedal.get_id();
            let available_parameters = pedal.get_parameters();

            for (rank, param_name) in PRIORITISE_PARAMETERS.iter().enumerate() {
                if available_parameters.contains_key(*param_name) {
                    prioritised.push((rank, pedal_order, pedal_id, (*param_name).to_string()));
                }
            }

            let mut remaining_parameters: Vec<&str> = available_parameters
                .iter()
                .filter(|(param_name, parameter)| {
                    !PRIORITISE_PARAMETERS.contains(&param_name.as_str())
                        && matches!(
                            &parameter.value,
                            PedalParameterValue::Float(_) | PedalParameterValue::Int(_)
                        )
                        && parameter.to_range().is_some()
                })
                .map(|(param_name, _parameter)| param_name.as_str())
                .collect();
            remaining_parameters.sort_unstable();

            fallback_parameters.extend(
                remaining_parameters
                    .into_iter()
                    .map(|param_name| (pedal_id, param_name.to_string())),
            );
        }

        prioritised.sort_by_key(|(rank, pedal_order, _pedal_id, _param_name)| {
            (*rank, *pedal_order)
        });

        let mut parameters: Vec<(u32, String)> = prioritised
            .into_iter()
            .map(|(_rank, _pedal_order, pedal_id, param_name)| (pedal_id, param_name))
            .collect();

        parameters.append(&mut fallback_parameters);
        parameters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pedals::{Delay, Fuzz, Overdrive};

    fn board(pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard::from_pedals("Test Pedalboard".to_string(), pedals)
    }

    /// The pedal index and parameter name which a knob at `index` controls.
    fn knob_target(board: &Pedalboard, index: usize) -> Option<(usize, String)> {
        let path =
            board.sensible_parameter_from_index(index, &FloatSettingUpdate::Relative(0.1))?;
        let pedal_index = board
            .pedals
            .iter()
            .position(|pedal| pedal.get_id() == path.pedal_id)
            .expect("a sensible parameter should belong to a pedal of this pedalboard");

        Some((pedal_index, path.parameter_name))
    }

    /// The index of the pedal which a footswitch at `index` toggles.
    fn toggle_target(board: &Pedalboard, index: usize) -> Option<usize> {
        let path = board.sensible_parameter_from_index(index, &FloatSettingUpdate::FlipFlop)?;
        assert_eq!(path.parameter_name, "Active");

        board
            .pedals
            .iter()
            .position(|pedal| pedal.get_id() == path.pedal_id)
    }

    #[test]
    fn the_first_knob_controls_the_first_parameter_worth_controlling() {
        let board = board(vec![
            Pedal::Overdrive(Overdrive::new()),
            Pedal::Overdrive(Overdrive::new()),
        ]);

        // The parameters which matter most come first ('Drive', then 'Level', then 'Tone' on an
        // overdrive) across the whole pedalboard, and parameters of equal importance are then
        // ordered by the pedals' positions on the pedalboard
        assert_eq!(knob_target(&board, 0), Some((0, "Drive".to_string())));
        assert_eq!(knob_target(&board, 1), Some((1, "Drive".to_string())));
        assert_eq!(knob_target(&board, 2), Some((0, "Level".to_string())));
        assert_eq!(knob_target(&board, 3), Some((1, "Level".to_string())));
        assert_eq!(knob_target(&board, 4), Some((0, "Tone".to_string())));
        assert_eq!(knob_target(&board, 5), Some((1, "Tone".to_string())));
    }

    #[test]
    fn a_prioritised_parameter_on_a_later_pedal_beats_a_lesser_one_on_an_earlier_pedal() {
        // The delay only offers 'Dry/Wet', while the fuzz offers 'Gain', 'Level' and 'Dry/Wet'
        let board = board(vec![Pedal::Delay(Delay::new()), Pedal::Fuzz(Fuzz::new())]);

        // 'Gain' outranks 'Dry/Wet', so the later fuzz is reached before the earlier delay
        assert_eq!(knob_target(&board, 0), Some((1, "Gain".to_string())));
        assert_eq!(knob_target(&board, 1), Some((1, "Level".to_string())));
        assert_eq!(knob_target(&board, 2), Some((0, "Dry/Wet".to_string())));
        assert_eq!(knob_target(&board, 3), Some((1, "Dry/Wet".to_string())));
    }

    #[test]
    fn a_footswitch_toggles_a_pedal_which_has_an_active_parameter() {
        // Pedals can be built without an 'Active' parameter, for example by a plugin which
        // doesn't report one
        let mut not_toggleable = Overdrive::new();
        not_toggleable.get_parameters_mut().remove("Active");

        let board = board(vec![
            Pedal::Overdrive(not_toggleable),
            Pedal::Overdrive(Overdrive::new()),
        ]);

        // The pedal without an 'Active' parameter is skipped rather than swallowing an index
        assert_eq!(toggle_target(&board, 0), Some(1));
        assert_eq!(toggle_target(&board, 1), None);
    }
}
