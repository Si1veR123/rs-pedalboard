use rs_pedalboard::pedalboard::Pedalboard;
use rs_pedalboard::set::Set;
use rs_pedalboard::pedals::bypass_pedal::BypassPedal;

fn main() {
    let mut set = Set::default();
    let mut peddleboard = Pedalboard::default();
    peddleboard.pedals.push(Box::new(BypassPedal::default()));
    set.pedalboards.push(peddleboard);
    let mut buffer = [0.0; 10];
    set.process_audio(&mut buffer);
    println!("{:?}", buffer);
}
