use rs_pedalboard::pedalboard::Pedalboard;


pub fn unique_pedalboard_name(mut name: String, pedalboards: &[Pedalboard]) -> String {
    let mut i = 1;
    while pedalboards.iter().any(|pedalboard| pedalboard.name == name) {
        if i == 1 {
            name.push_str("_1");
        } else {
            name.pop();
            name.push_str(&i.to_string());
        }

        i += 1;
    }
    name
}
