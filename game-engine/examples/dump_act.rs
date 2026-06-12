use game_engine::infrastructure::ro_formats::act::parse_act;

fn dump(label: &str, act: &game_engine::infrastructure::ro_formats::act::RoAction, action: usize) {
    let Some(seq) = act.actions.get(action) else {
        println!(
            "{label} action {action}: MISSING (total {})",
            act.actions.len()
        );
        return;
    };
    println!(
        "{label} action {action}: {} frames, delay {}",
        seq.animations.len(),
        seq.delay
    );
    for (i, anim) in seq.animations.iter().enumerate() {
        let layer = anim
            .layers
            .iter()
            .find(|l| l.sprite_index >= 0)
            .map(|l| format!("pos=({}, {}) mirror={}", l.pos[0], l.pos[1], l.is_mirror))
            .unwrap_or_else(|| "no layer".to_string());
        let attach = anim
            .positions
            .first()
            .map(|p| format!("attach=({}, {})", p.x, p.y))
            .unwrap_or_else(|| "no attach".to_string());
        println!("  frame {i}: {layer} | {attach}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let body_data = std::fs::read(&args[1]).expect("body act");
    let head_data = std::fs::read(&args[2]).expect("head act");

    let body = parse_act(&body_data).expect("parse body");
    let head = parse_act(&head_data).expect("parse head");

    println!(
        "body: {} actions | head: {} actions",
        body.actions.len(),
        head.actions.len()
    );

    for action in [0, 2, 8, 10, 88, 90] {
        dump("BODY", &body, action);
        dump("HEAD", &head, action);
        println!();
    }
}
